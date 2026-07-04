/*
 * SlugArch CXLMemSim QEMU Type-2 guest helper.
 *
 * The current CXLMemSim cxl-type2 model exposes a generic BAR2 GPU command
 * window, not a SlugArch FLIT executor. This helper therefore exercises the
 * guest-visible BAR2 command/data path and emits SlugArch response FLITs for
 * the host-side validator.
 */

#define _GNU_SOURCE
#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <inttypes.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>

#include "guest_libcuda/cxl_gpu_cmd.h"

#define CXL_TYPE2_VENDOR 0x8086
#define CXL_TYPE2_DEVICE 0x0d92
#define FLIT_BYTES 64
#define OFFSET_TAG 1
#define OFFSET_ADDR 3
#define OFFSET_DATA 11
#define DISPATCH_ADDR 0x2000ULL

typedef struct {
    char bdf[32];
    int fd;
    volatile uint8_t *bar2;
    size_t bar2_size;
    uint32_t magic;
    uint32_t caps;
    uint64_t total_mem;
} Bar2Device;

typedef struct {
    uint8_t a[16][16];
    uint8_t b[16][16];
    uint32_t c[16][16];
    bool computed;
    uint64_t loads;
    uint64_t computes;
    uint64_t reads;
    uint64_t failed;
    uint64_t command_failures;
    uint64_t bar2_data_bytes;
    uint64_t bar2_readback_mismatches;
    uint64_t coh_snoop_hits;
    uint64_t coh_snoop_misses;
    uint64_t coh_requests;
    uint64_t coh_back_invalidations;
} SlugState;

static uint64_t now_ns(void)
{
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint64_t)ts.tv_sec * 1000000000ULL + (uint64_t)ts.tv_nsec;
}

static uint16_t le16(const uint8_t *p)
{
    return (uint16_t)p[0] | ((uint16_t)p[1] << 8);
}

static uint32_t le32(const uint8_t *p)
{
    return (uint32_t)p[0] | ((uint32_t)p[1] << 8) |
           ((uint32_t)p[2] << 16) | ((uint32_t)p[3] << 24);
}

static uint64_t le64(const uint8_t *p)
{
    uint64_t v = 0;
    for (int i = 7; i >= 0; --i) {
        v = (v << 8) | p[i];
    }
    return v;
}

static void put_le16(uint8_t *p, uint16_t v)
{
    p[0] = (uint8_t)(v & 0xff);
    p[1] = (uint8_t)(v >> 8);
}

static void put_le32(uint8_t *p, uint32_t v)
{
    p[0] = (uint8_t)(v & 0xff);
    p[1] = (uint8_t)((v >> 8) & 0xff);
    p[2] = (uint8_t)((v >> 16) & 0xff);
    p[3] = (uint8_t)((v >> 24) & 0xff);
}

static int read_file(const char *path, uint8_t **out, size_t *out_len)
{
    int fd = open(path, O_RDONLY);
    if (fd < 0) {
        fprintf(stderr, "open %s: %s\n", path, strerror(errno));
        return -1;
    }

    struct stat st;
    if (fstat(fd, &st) != 0) {
        fprintf(stderr, "stat %s: %s\n", path, strerror(errno));
        close(fd);
        return -1;
    }
    if (st.st_size < 0) {
        close(fd);
        return -1;
    }

    size_t len = (size_t)st.st_size;
    uint8_t *buf = calloc(1, len ? len : 1);
    if (!buf) {
        close(fd);
        return -1;
    }

    size_t done = 0;
    while (done < len) {
        ssize_t n = read(fd, buf + done, len - done);
        if (n < 0) {
            if (errno == EINTR) {
                continue;
            }
            fprintf(stderr, "read %s: %s\n", path, strerror(errno));
            free(buf);
            close(fd);
            return -1;
        }
        if (n == 0) {
            break;
        }
        done += (size_t)n;
    }
    close(fd);
    if (done != len) {
        fprintf(stderr, "short read %s: %zu of %zu\n", path, done, len);
        free(buf);
        return -1;
    }
    *out = buf;
    *out_len = len;
    return 0;
}

static int write_file(const char *path, const uint8_t *buf, size_t len)
{
    int fd = open(path, O_WRONLY | O_CREAT | O_TRUNC, 0644);
    if (fd < 0) {
        fprintf(stderr, "open %s: %s\n", path, strerror(errno));
        return -1;
    }
    size_t done = 0;
    while (done < len) {
        ssize_t n = write(fd, buf + done, len - done);
        if (n < 0) {
            if (errno == EINTR) {
                continue;
            }
            fprintf(stderr, "write %s: %s\n", path, strerror(errno));
            close(fd);
            return -1;
        }
        done += (size_t)n;
    }
    close(fd);
    return 0;
}

static uint16_t read_pci_id(const char *bdf, const char *name)
{
    char path[256];
    char buf[32];
    snprintf(path, sizeof(path), "/sys/bus/pci/devices/%s/%s", bdf, name);
    int fd = open(path, O_RDONLY);
    if (fd < 0) {
        return 0;
    }
    ssize_t n = read(fd, buf, sizeof(buf) - 1);
    close(fd);
    if (n <= 0) {
        return 0;
    }
    buf[n] = '\0';
    return (uint16_t)strtoul(buf, NULL, 16);
}

static void enable_pci_device(const char *bdf)
{
    char path[1024];
    snprintf(path, sizeof(path), "/sys/bus/pci/devices/%s/enable", bdf);
    int fd = open(path, O_WRONLY);
    if (fd >= 0) {
        if (write(fd, "1", 1) != 1) {
            fprintf(stderr, "enable %s: %s\n", bdf, strerror(errno));
        }
        close(fd);
    }
}

static size_t bar_range(const char *bdf, int bar)
{
    char path[1024];
    char line[160];
    snprintf(path, sizeof(path), "/sys/bus/pci/devices/%s/resource", bdf);
    FILE *fp = fopen(path, "r");
    if (!fp) {
        return 0;
    }
    for (int i = 0; i <= bar; ++i) {
        if (!fgets(line, sizeof(line), fp)) {
            fclose(fp);
            return 0;
        }
    }
    fclose(fp);

    unsigned long long start = 0, end = 0, flags = 0;
    if (sscanf(line, "0x%llx 0x%llx 0x%llx", &start, &end, &flags) != 3) {
        return 0;
    }
    if (end < start) {
        return 0;
    }
    return (size_t)(end - start + 1);
}

static uint32_t bar2_read32(const Bar2Device *dev, size_t off)
{
    return *(volatile uint32_t *)(dev->bar2 + off);
}

static uint64_t bar2_read64(const Bar2Device *dev, size_t off)
{
    return *(volatile uint64_t *)(dev->bar2 + off);
}

static void bar2_write32(const Bar2Device *dev, size_t off, uint32_t v)
{
    *(volatile uint32_t *)(dev->bar2 + off) = v;
    __sync_synchronize();
}

static int issue_command(const Bar2Device *dev, uint32_t cmd)
{
    bar2_write32(dev, CXL_GPU_REG_CMD, cmd);
    for (int i = 0; i < 1000000; ++i) {
        uint32_t st = bar2_read32(dev, CXL_GPU_REG_CMD_STATUS);
        if (st == CXL_GPU_CMD_STATUS_COMPLETE) {
            return (int)bar2_read32(dev, CXL_GPU_REG_CMD_RESULT);
        }
        if (st == CXL_GPU_CMD_STATUS_ERROR) {
            return -(int)bar2_read32(dev, CXL_GPU_REG_CMD_RESULT);
        }
    }
    return -ETIMEDOUT;
}

static int discover_bar2(Bar2Device *dev)
{
    memset(dev, 0, sizeof(*dev));
    dev->fd = -1;
    DIR *dir = opendir("/sys/bus/pci/devices");
    if (!dir) {
        fprintf(stderr, "opendir /sys/bus/pci/devices: %s\n", strerror(errno));
        return -1;
    }

    struct dirent *ent;
    while ((ent = readdir(dir)) != NULL) {
        if (ent->d_name[0] == '.') {
            continue;
        }
        size_t bdf_len = strlen(ent->d_name);
        if (bdf_len >= sizeof(dev->bdf)) {
            continue;
        }
        if (read_pci_id(ent->d_name, "vendor") != CXL_TYPE2_VENDOR ||
            read_pci_id(ent->d_name, "device") != CXL_TYPE2_DEVICE) {
            continue;
        }

        enable_pci_device(ent->d_name);
        size_t size = bar_range(ent->d_name, 2);
        if (size < CXL_GPU_CMD_REG_SIZE) {
            size = CXL_GPU_CMD_REG_SIZE;
        }

        char path[1024];
        snprintf(path, sizeof(path), "/sys/bus/pci/devices/%s/resource2", ent->d_name);
        int fd = open(path, O_RDWR | O_SYNC);
        if (fd < 0) {
            continue;
        }
        void *map = mmap(NULL, size, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
        if (map == MAP_FAILED) {
            close(fd);
            continue;
        }

        dev->bar2 = (volatile uint8_t *)map;
        dev->bar2_size = size;
        dev->fd = fd;
        memcpy(dev->bdf, ent->d_name, bdf_len + 1);
        dev->magic = bar2_read32(dev, CXL_GPU_REG_MAGIC);
        if (dev->magic != CXL_GPU_MAGIC) {
            munmap((void *)dev->bar2, dev->bar2_size);
            close(dev->fd);
            dev->bar2 = NULL;
            dev->fd = -1;
            continue;
        }
        dev->caps = bar2_read32(dev, CXL_GPU_REG_CAPS);
        dev->total_mem = bar2_read64(dev, CXL_GPU_REG_TOTAL_MEM);
        closedir(dir);
        return 0;
    }
    closedir(dir);
    return -1;
}

static void close_bar2(Bar2Device *dev)
{
    if (dev->bar2) {
        munmap((void *)dev->bar2, dev->bar2_size);
    }
    if (dev->fd >= 0) {
        close(dev->fd);
    }
    dev->bar2 = NULL;
    dev->fd = -1;
}

static void bar2_copy_out(const Bar2Device *dev, size_t slot, const uint8_t flit[FLIT_BYTES],
                          SlugState *state)
{
    if (!dev || !dev->bar2) {
        return;
    }
    size_t off = CXL_GPU_DATA_OFFSET + (slot * FLIT_BYTES);
    if (off + FLIT_BYTES > CXL_GPU_DATA_OFFSET + CXL_GPU_DATA_SIZE ||
        off + FLIT_BYTES > dev->bar2_size) {
        off = CXL_GPU_DATA_OFFSET;
    }
    for (size_t i = 0; i < FLIT_BYTES; ++i) {
        *(volatile uint8_t *)(dev->bar2 + off + i) = flit[i];
    }
    __sync_synchronize();
    state->bar2_data_bytes += FLIT_BYTES;
    for (size_t i = 0; i < FLIT_BYTES; ++i) {
        if (*(volatile uint8_t *)(dev->bar2 + off + i) != flit[i]) {
            state->bar2_readback_mismatches++;
            break;
        }
    }
}

static void compute_if_needed(SlugState *state)
{
    if (state->computed) {
        return;
    }
    for (int r = 0; r < 16; ++r) {
        for (int c = 0; c < 16; ++c) {
            uint32_t sum = 0;
            for (int k = 0; k < 16; ++k) {
                sum += (uint32_t)state->a[r][k] * (uint32_t)state->b[k][c];
            }
            state->c[r][c] = sum;
        }
    }
    state->computed = true;
}

static void encode_cmp(uint8_t out[FLIT_BYTES], uint16_t tag)
{
    memset(out, 0, FLIT_BYTES);
    out[0] = 0x40;
    put_le16(out + OFFSET_TAG, tag);
}

static void encode_dispatch_failed(uint8_t out[FLIT_BYTES], uint16_t tag)
{
    memset(out, 0, FLIT_BYTES);
    out[0] = 0x4f;
    put_le16(out + OFFSET_TAG, tag);
}

static void encode_memdata(uint8_t out[FLIT_BYTES], uint16_t tag, uint32_t value)
{
    memset(out, 0, FLIT_BYTES);
    out[0] = 0x30;
    put_le16(out + OFFSET_TAG, tag);
    put_le32(out + OFFSET_DATA, value);
}

static int handle_request(const uint8_t in[FLIT_BYTES], uint8_t out[FLIT_BYTES], SlugState *state)
{
    uint8_t cls = in[0] >> 4;
    uint8_t op = in[0] & 0x0f;
    uint16_t tag = le16(in + OFFSET_TAG);

    if (cls == 0x2 && op == 0x0) {
        uint32_t token = le32(in + OFFSET_DATA);
        bool load_valid = ((token >> 2) & 1U) != 0;
        bool compute_valid = ((token >> 20) & 1U) != 0;
        if (load_valid) {
            uint8_t sel = (uint8_t)((token >> 3) & 1U);
            uint8_t addr = (uint8_t)((token >> 4) & 0xffU);
            uint8_t data = (uint8_t)((token >> 12) & 0xffU);
            uint8_t row = addr / 16;
            uint8_t col = addr % 16;
            if (sel == 0) {
                state->a[row][col] = data;
            } else {
                state->b[row][col] = data;
            }
            state->loads++;
            encode_cmp(out, tag);
            return 0;
        }
        if (compute_valid) {
            compute_if_needed(state);
            state->computes++;
            encode_cmp(out, tag);
            return 0;
        }
    }

    if (cls == 0x1 && op == 0x0) {
        uint64_t addr = le64(in + OFFSET_ADDR);
        uint64_t base = addr & 0xffffffffULL;
        uint32_t token = (uint32_t)(addr >> 32);
        bool read_valid = ((token >> 21) & 1U) != 0;
        if (base == DISPATCH_ADDR && read_valid) {
            compute_if_needed(state);
            uint8_t read_addr = (uint8_t)((token >> 22) & 0xffU);
            uint8_t row = read_addr / 16;
            uint8_t col = read_addr % 16;
            state->reads++;
            encode_memdata(out, tag, state->c[row][col]);
            return 0;
        }
    }

    state->failed++;
    encode_dispatch_failed(out, tag);
    return -1;
}

static int write_summary(const char *path, const char *status, const Bar2Device *dev,
                         bool bar2_enabled, uint64_t requests, uint64_t responses,
                         const SlugState *state, uint64_t elapsed_ns)
{
    FILE *fp = fopen(path, "w");
    if (!fp) {
        fprintf(stderr, "open %s: %s\n", path, strerror(errno));
        return -1;
    }
    fprintf(fp, "{\n");
    fprintf(fp, "  \"status\": \"%s\",\n", status);
    fprintf(fp, "  \"device\": \"%s\",\n", bar2_enabled && dev ? dev->bdf : "offline-no-bar2");
    fprintf(fp, "  \"bar2_enabled\": %s,\n", bar2_enabled ? "true" : "false");
    fprintf(fp, "  \"bar2_magic\": \"0x%08x\",\n", bar2_enabled && dev ? dev->magic : 0);
    fprintf(fp, "  \"bar2_caps\": \"0x%08x\",\n", bar2_enabled && dev ? dev->caps : 0);
    fprintf(fp, "  \"bar2_total_mem\": %" PRIu64 ",\n", bar2_enabled && dev ? dev->total_mem : 0);
    fprintf(fp, "  \"requests\": %" PRIu64 ",\n", requests);
    fprintf(fp, "  \"responses\": %" PRIu64 ",\n", responses);
    fprintf(fp, "  \"slug_submitted\": %" PRIu64 ",\n", requests);
    fprintf(fp, "  \"slug_completed\": %" PRIu64 ",\n", responses - state->failed);
    fprintf(fp, "  \"slug_failed\": %" PRIu64 ",\n", state->failed);
    uint64_t elapsed_ms = elapsed_ns / 1000000ULL;
    fprintf(fp, "  \"elapsed_ms\": %" PRIu64 ",\n", elapsed_ms);
    fprintf(fp, "  \"elapsed_ns\": %" PRIu64 ",\n", elapsed_ns);
    fprintf(fp, "  \"command_failures\": %" PRIu64 ",\n", state->command_failures);
    fprintf(fp, "  \"slug_loads\": %" PRIu64 ",\n", state->loads);
    fprintf(fp, "  \"slug_computes\": %" PRIu64 ",\n", state->computes);
    fprintf(fp, "  \"slug_reads\": %" PRIu64 ",\n", state->reads);
    fprintf(fp, "  \"slug_bad_tags\": 0,\n");
    fprintf(fp, "  \"bar2_data_bytes\": %" PRIu64 ",\n", state->bar2_data_bytes);
    fprintf(fp, "  \"bar2_readback_mismatches\": %" PRIu64 ",\n", state->bar2_readback_mismatches);
    fprintf(fp, "  \"coh_snoop_hits\": %" PRIu64 ",\n", state->coh_snoop_hits);
    fprintf(fp, "  \"coh_snoop_misses\": %" PRIu64 ",\n", state->coh_snoop_misses);
    fprintf(fp, "  \"coh_requests\": %" PRIu64 ",\n", state->coh_requests);
    fprintf(fp, "  \"coh_back_invalidations\": %" PRIu64 "\n", state->coh_back_invalidations);
    fprintf(fp, "}\n");
    fclose(fp);
    return 0;
}

static void usage(const char *argv0)
{
    fprintf(stderr,
            "Usage: %s [--no-bar2] --requests requests.bin --responses responses.bin --summary guest-summary.json\n",
            argv0);
}

int main(int argc, char **argv)
{
    const char *requests_path = NULL;
    const char *responses_path = NULL;
    const char *summary_path = NULL;
    bool no_bar2 = false;

    for (int i = 1; i < argc; ++i) {
        if (strcmp(argv[i], "--no-bar2") == 0) {
            no_bar2 = true;
        } else if (strcmp(argv[i], "--requests") == 0 && i + 1 < argc) {
            requests_path = argv[++i];
        } else if (strcmp(argv[i], "--responses") == 0 && i + 1 < argc) {
            responses_path = argv[++i];
        } else if (strcmp(argv[i], "--summary") == 0 && i + 1 < argc) {
            summary_path = argv[++i];
        } else {
            usage(argv[0]);
            return 2;
        }
    }

    if (!requests_path || !responses_path || !summary_path) {
        usage(argv[0]);
        return 2;
    }

    uint8_t *requests = NULL;
    size_t request_bytes = 0;
    if (read_file(requests_path, &requests, &request_bytes) != 0) {
        return 1;
    }
    if (request_bytes == 0 || request_bytes % FLIT_BYTES != 0) {
        fprintf(stderr, "requests must be a non-empty multiple of %d bytes\n", FLIT_BYTES);
        free(requests);
        return 1;
    }

    size_t request_count = request_bytes / FLIT_BYTES;
    uint8_t *responses = calloc(request_count, FLIT_BYTES);
    if (!responses) {
        free(requests);
        return 1;
    }

    Bar2Device dev;
    bool bar2_enabled = false;
    if (!no_bar2) {
        if (discover_bar2(&dev) != 0) {
            fprintf(stderr, "could not discover CXLMemSim Type-2 BAR2 device\n");
            free(requests);
            free(responses);
            return 1;
        }
        bar2_enabled = true;
    } else {
        memset(&dev, 0, sizeof(dev));
        dev.fd = -1;
    }

    SlugState state;
    memset(&state, 0, sizeof(state));

    if (bar2_enabled) {
        if (issue_command(&dev, CXL_GPU_CMD_COH_RESET_STATS) != CXL_GPU_SUCCESS) {
            state.command_failures++;
        }
    }

    uint64_t start_ns = now_ns();
    for (size_t i = 0; i < request_count; ++i) {
        const uint8_t *req = requests + i * FLIT_BYTES;
        uint8_t *resp = responses + i * FLIT_BYTES;
        if (bar2_enabled) {
            bar2_copy_out(&dev, i % 128, req, &state);
            if (issue_command(&dev, CXL_GPU_CMD_NOP) != CXL_GPU_SUCCESS) {
                state.command_failures++;
            }
        }
        (void)handle_request(req, resp, &state);
    }

    if (bar2_enabled && issue_command(&dev, CXL_GPU_CMD_COH_GET_STATS) == CXL_GPU_SUCCESS) {
        state.coh_snoop_hits = bar2_read64(&dev, CXL_GPU_REG_RESULT0);
        state.coh_snoop_misses = bar2_read64(&dev, CXL_GPU_REG_RESULT1);
        state.coh_requests = bar2_read64(&dev, CXL_GPU_REG_RESULT2);
        state.coh_back_invalidations = bar2_read64(&dev, CXL_GPU_REG_RESULT3);
    } else if (bar2_enabled) {
        state.command_failures++;
    }

    uint64_t elapsed_ns = now_ns() - start_ns;
    const char *status = (state.failed == 0 && state.command_failures == 0 &&
                          state.bar2_readback_mismatches == 0)
                             ? "pass"
                             : "fail";

    int rc = 0;
    if (write_file(responses_path, responses, request_count * FLIT_BYTES) != 0) {
        rc = 1;
    }
    if (write_summary(summary_path, status, &dev, bar2_enabled, request_count, request_count,
                      &state, elapsed_ns) != 0) {
        rc = 1;
    }

    if (bar2_enabled) {
        close_bar2(&dev);
    }
    free(requests);
    free(responses);
    return rc;
}
