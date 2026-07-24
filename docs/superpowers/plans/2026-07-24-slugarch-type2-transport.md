# SlugArch Type-2 CXL.mem Transport Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the approved, versioned, synchronous QEMU Type-2 CXL.mem transport to a server-authoritative CXLMemSim backing store, then prove it with protocol tests, a one-target CFMWS path, a corrected Type-2 DVSEC, a compatible guest-kernel memdev path, and a two-way devdax/oracle sentinel exchange.

**Architecture:** A default-off `SLT2` protocol mode is implemented independently on the CXLMemSim and QEMU sides and cross-checked with the same golden frame. CXLMemSim owns the bytes, counters, server sequence, and gzip JSONL evidence; QEMU serializes one request/response transaction per CFMWS access, applies the returned delay, and emits joinable path/delay evidence. The only supported CFMWS shape is one fixed window, one host-bridge target, one direct root port, one Type-2 endpoint, one-way interleave, and a zero-based 256 MiB DPA range.

**Tech Stack:** C++20, C11, CMake/CTest, QEMU C/Meson/Ninja/qtest, POSIX sockets and `poll`, CRC-32C, OpenSSL SHA-256, zlib gzip streams, Linux CXL core and the custom `cxl_type2_accel` module, `cxl-cli`, `daxctl`, Bash, QEMU TCG.

---

## Scope and non-negotiable proof boundary

This plan implements only the transport and its live proof gates. It does not run the four-latency, five-boot paper campaign and does not change the manuscript.

The implementation is acceptable only when all of the following are true:

- The legacy CXLMemSim TCP protocol and legacy asynchronous Type-2 behavior remain the defaults.
- `SLT2` mode fails closed on framing, CRC, version, request-ID, status, range, timeout, or partial-I/O errors.
- A successful Type-2 CFMWS read returns bytes received from CXLMemSim, not QEMU-local RAM or cache.
- A Type-2 CFMWS write is not successful until CXLMemSim has committed it.
- Every successful response has exactly one server completion event and one QEMU delay event with matching client ID, request ID, server sequence, DPA, length, operation, and payload digest.
- The server advertises and enforces exactly 268,435,456 data bytes. It never modulo-wraps an out-of-range DPA.
- QEMU exposes one active zero-based 256 MiB DVSEC memory range. Cache capacity appears only in `cap2`.
- The live path uses `/dev/dax*` created from the CFMWS/region path; neither PCI `resource4` nor a BAR4 mapping is used.
- The work line for the sentinel is at DPA 80 MiB, outside `[0,64 MiB)` and `[192,256 MiB)`.
- A host oracle write is visible to the guest through devdax, and a guest devdax write is visible to the oracle.
- The QEMU successful-RPC count equals the server completed count, while BAR4-overlay, local-shadow-completion, and local-cache-completion counts remain zero.

## Current source state that must be preserved

| Tree | Current state | Consequence |
| --- | --- | --- |
| `/home/victoryang00/CXLMemSim` | dirty outer repository; QEMU submodule modified | Never build or edit the user checkout in place. |
| `/home/victoryang00/CXLMemSim/lib/qemu` | detached at `904f0a3cb2a56ca58a66f2c5507a0b2b9eb67b10`; seven staged CXL/GPU files plus unrelated generated-header and ROM dirt | Apply the staged CXL patch to the isolated clone; archive but do not apply generated-header, package, or ROM dirt. |
| `/home/victoryang00/cxl` | dirty CXL core and Type-2 driver, plus required untracked identity headers/tests | Capture the tracked diff and the four required untracked files before changing the isolated clone. |
| `/home/victoryang00/CXLMemSim/build/qemu1.img` | raw ext4 base image; contains a March 2 `cxl_type2_accel.ko` without the current parameters | Treat it as immutable input. Produce a copied experiment image containing a matching kernel module set. |

The current QEMU Type-2 Register Locator advertises no device-register/mailbox block. The current dirty kernel driver nevertheless requires two mailbox `Identify` replies. The kernel task below deliberately resolves this by deriving QEMU capacity from two identical active-DVSEC snapshots; it does not add an unplanned Type-3 mailbox model to QEMU.

## File structure

### CXLMemSim outer repository

- Create `include/slugarch_type2_protocol.h`
  - Protocol constants, typed frames, codec results, exact-deadline I/O API.
- Create `src/slugarch_type2_protocol.cpp`
  - Little-endian codec, CRC-32C, fixed-length validation, `poll`-based exact I/O.
- Create `include/slugarch_type2_server.h`
  - Server service, per-client counters, backend and evidence interfaces.
- Create `src/slugarch_type2_server.cpp`
  - HELLO/ACK, memory operations, counter snapshots, server sequence, gzip JSONL.
- Create `tools/slugarch_type2_oracle.cpp`
  - Oracle READ, WRITE, counter snapshot, and deterministic sentinel commands.
- Create `tests/test_slugarch_type2_protocol.cpp`
  - Golden bytes, endian, CRC, malformed frame, short I/O, EOF, and timeout tests.
- Create `tests/test_slugarch_type2_server.cpp`
  - Handshake, authority, strict range, role, counters, and evidence tests.
- Modify `include/shared_memory_manager.h`
  - Distinguish logical data capacity from total mapping bytes.
- Modify `src/shared_memory_manager.cc`
  - Allocate header plus exact logical capacity and expose exact data size.
- Modify `src/main_server.cc`
  - Add default-off CLI options and route accepted TCP sockets to the new service.
- Modify `CMakeLists.txt`
  - Build protocol/service/oracle/tests and link OpenSSL Crypto plus zlib.
- Create `qemu_integration/slugarch_type2_protocol_smoke.sh`
  - Server-only oracle/QEMU-role protocol smoke.
- Create `qemu_integration/slugarch_type2_no_guest_smoke.sh`
  - QEMU realization/handshake and legacy-layout rejection smoke.
- Create `qemu_integration/slugarch_type2_guest_setup.sh`
  - Exact one-memdev region/devdax setup and machine-readable topology output.
- Create `qemu_integration/slugarch_type2_dax_sentinel.c`
  - Guest DAX read/verify and write commands for the two-way sentinel.
- Create `qemu_integration/slugarch_type2_live_sentinel.sh`
  - Immutable-image copy, server/QEMU boot, guest setup, oracle exchange, and joins.

### QEMU submodule

- Create `include/hw/cxl/slugarch_type2_protocol.h`
  - QEMU-side wire constants, typed messages, transport and delay API.
- Create `hw/cxl/slugarch_type2_protocol.c`
  - QEMU codec, exact-deadline socket I/O, CRC, and delay implementation.
- Create `tests/unit/test-cxl-type2-wire.c`
  - The same golden frame and malformed/deadline cases as the server.
- Create `tests/unit/test-cxl-type2-route.c`
  - Pure one-target topology/range validation tests.
- Modify `include/hw/cxl/cxl_type2.h`
  - Synchronous state, counters, and direct CFMWS entry points.
- Modify `hw/cxl/cxl_type2.c`
  - Property, handshake, synchronous RPC, evidence, delay, overlay observers, DVSEC.
- Modify `hw/cxl/cxl-host.c`
  - Direct Type-2 discovery and CFMWS dispatch.
- Modify `hw/cxl/meson.build`
  - Compile the protocol source.
- Modify `tests/unit/meson.build`
  - Build the two new unit tests.
- Modify `tests/qtest/cxl-test.c`
  - Type-2 realization and DVSEC regression coverage.

### Guest kernel

- Create `drivers/cxl/type2_dvsec_capacity.h`
  - Pure validation of repeatable DVSEC capacity snapshots.
- Create `tools/testing/cxl/type2_dvsec_capacity_test.c`
  - Userspace tests for zero, mismatch, wrong-size, and valid snapshots.
- Modify `drivers/cxl/cxl_type2_accel.c`
  - QEMU-only repeatable active-DVSEC capacity discovery.
- Modify `drivers/cxl/core/hdm.c`
  - Reserve zero-based DPA capacity while allowing the region flow to software-commit the endpoint HPA range.

---

### Task 1: Capture dirty inputs and create isolated implementation clones

**Files:**

- Create outside the user trees: `/tmp/slugarch-type2-transport-src/source-capture/`
- Create isolated clone: `/tmp/slugarch-type2-transport-src/CXLMemSim`
- Create isolated QEMU clone: `/tmp/slugarch-type2-transport-src/CXLMemSim/lib/qemu`
- Create isolated kernel clone: `/tmp/slugarch-type2-transport-src/cxl`

- [ ] **Step 1: Fail if the fixed isolation root already exists**

Run:

```bash
test ! -e /tmp/slugarch-type2-transport-src
mkdir -p /tmp/slugarch-type2-transport-src/source-capture/patches
mkdir -p /tmp/slugarch-type2-transport-src/source-capture/status
mkdir -p /tmp/slugarch-type2-transport-src/source-capture/untracked/kernel
```

Expected: every command exits `0`. If the first command fails, stop and choose a new explicitly recorded root; never delete an unidentified prior capture.

- [ ] **Step 2: Record commits, status, submodule state, and tracked patches**

Run:

```bash
git -C /home/victoryang00/CXLMemSim rev-parse HEAD > /tmp/slugarch-type2-transport-src/source-capture/status/cxlmemsim.head
git -C /home/victoryang00/CXLMemSim status --porcelain=v2 > /tmp/slugarch-type2-transport-src/source-capture/status/cxlmemsim.status
git -C /home/victoryang00/CXLMemSim submodule status > /tmp/slugarch-type2-transport-src/source-capture/status/cxlmemsim.submodules
git -C /home/victoryang00/CXLMemSim diff --binary -- qemu_integration/launch_qemu_cxl_usernet.sh qemu_integration/setup_cxl_numa.sh > /tmp/slugarch-type2-transport-src/source-capture/patches/cxlmemsim.relevant-worktree.patch
git -C /home/victoryang00/CXLMemSim diff --binary --cached -- qemu_integration/launch_qemu_cxl_usernet.sh qemu_integration/setup_cxl_numa.sh > /tmp/slugarch-type2-transport-src/source-capture/patches/cxlmemsim.relevant-index.patch

git -C /home/victoryang00/CXLMemSim/lib/qemu rev-parse HEAD > /tmp/slugarch-type2-transport-src/source-capture/status/qemu.head
git -C /home/victoryang00/CXLMemSim/lib/qemu status --porcelain=v2 > /tmp/slugarch-type2-transport-src/source-capture/status/qemu.status
git -C /home/victoryang00/CXLMemSim/lib/qemu diff --binary --cached > /tmp/slugarch-type2-transport-src/source-capture/patches/qemu.index.patch
git -C /home/victoryang00/CXLMemSim/lib/qemu diff --binary > /tmp/slugarch-type2-transport-src/source-capture/patches/qemu.worktree.patch

git -C /home/victoryang00/cxl rev-parse HEAD > /tmp/slugarch-type2-transport-src/source-capture/status/kernel.head
git -C /home/victoryang00/cxl status --porcelain=v2 > /tmp/slugarch-type2-transport-src/source-capture/status/kernel.status
git -C /home/victoryang00/cxl diff --binary --cached > /tmp/slugarch-type2-transport-src/source-capture/patches/kernel.index.patch
git -C /home/victoryang00/cxl diff --binary > /tmp/slugarch-type2-transport-src/source-capture/patches/kernel.worktree.patch
```

Expected: the three `.head` files contain one 40-hex commit each; status and
patch files exist even when empty. The outer repository has unrelated
unreadable workload files, so do not run a broad `git diff` that would fail
before capturing the two relevant launch/setup files. The full porcelain
status remains the inventory for all out-of-scope dirt.

- [ ] **Step 3: Capture required untracked kernel inputs and the exact config**

Run:

```bash
install -D -m 0644 /home/victoryang00/cxl/drivers/cxl/capcxl_identity.h /tmp/slugarch-type2-transport-src/source-capture/untracked/kernel/drivers/cxl/capcxl_identity.h
install -D -m 0644 /home/victoryang00/cxl/drivers/cxl/tmatmul_identity.h /tmp/slugarch-type2-transport-src/source-capture/untracked/kernel/drivers/cxl/tmatmul_identity.h
install -D -m 0644 /home/victoryang00/cxl/tools/testing/cxl/capcxl_identity_test.c /tmp/slugarch-type2-transport-src/source-capture/untracked/kernel/tools/testing/cxl/capcxl_identity_test.c
install -D -m 0644 /home/victoryang00/cxl/tools/testing/cxl/tmatmul_identity_test.c /tmp/slugarch-type2-transport-src/source-capture/untracked/kernel/tools/testing/cxl/tmatmul_identity_test.c
install -D -m 0644 /home/victoryang00/cxl/.config /tmp/slugarch-type2-transport-src/source-capture/kernel.config
sha256sum /tmp/slugarch-type2-transport-src/source-capture/kernel.config > /tmp/slugarch-type2-transport-src/source-capture/kernel.config.sha256
```

Expected: five copied files and a valid SHA-256 line.

- [ ] **Step 4: Clone exact bases without touching the originals**

Run:

```bash
git clone --no-hardlinks /home/victoryang00/CXLMemSim /tmp/slugarch-type2-transport-src/CXLMemSim
rmdir /tmp/slugarch-type2-transport-src/CXLMemSim/lib/qemu
git clone --no-hardlinks /home/victoryang00/CXLMemSim/lib/qemu /tmp/slugarch-type2-transport-src/CXLMemSim/lib/qemu
git clone --no-hardlinks /home/victoryang00/cxl /tmp/slugarch-type2-transport-src/cxl
```

Expected: three independent `.git` directories exist. Running
`git -C /tmp/slugarch-type2-transport-src/CXLMemSim rev-parse --is-inside-work-tree`
and the equivalent command in the nested QEMU and kernel clones prints `true`.

- [ ] **Step 5: Reapply only the QEMU staged baseline and the complete kernel work**

Run:

```bash
git -C /tmp/slugarch-type2-transport-src/CXLMemSim/lib/qemu apply /tmp/slugarch-type2-transport-src/source-capture/patches/qemu.index.patch
git -C /tmp/slugarch-type2-transport-src/cxl apply /tmp/slugarch-type2-transport-src/source-capture/patches/kernel.index.patch
git -C /tmp/slugarch-type2-transport-src/cxl apply /tmp/slugarch-type2-transport-src/source-capture/patches/kernel.worktree.patch
cp -a /tmp/slugarch-type2-transport-src/source-capture/untracked/kernel/. /tmp/slugarch-type2-transport-src/cxl/
cp /tmp/slugarch-type2-transport-src/source-capture/kernel.config /tmp/slugarch-type2-transport-src/cxl/.config
```

Expected:

- QEMU has changes only in the seven staged CXL/GPU source files captured by `qemu.index.patch`.
- The QEMU generated Linux headers, `cuda-keyring` package, and dirty ROM submodules are absent.
- The kernel clone contains the captured CXL changes and four required identity files.
- The outer clone is clean except that its nested QEMU worktree reports modified content.

- [ ] **Step 6: Hash the complete capture before implementation**

Run:

```bash
find /tmp/slugarch-type2-transport-src/source-capture -type f -print0 | sort -z | xargs -0 sha256sum > /tmp/slugarch-type2-transport-src/source-capture/SHA256SUMS
sha256sum -c /tmp/slugarch-type2-transport-src/source-capture/SHA256SUMS
```

Expected: every entry prints `OK`.

---

### Task 2: Add the CXLMemSim `SLT2` codec and exact-deadline I/O

**Files:**

- Create: `/tmp/slugarch-type2-transport-src/CXLMemSim/include/slugarch_type2_protocol.h`
- Create: `/tmp/slugarch-type2-transport-src/CXLMemSim/src/slugarch_type2_protocol.cpp`
- Create: `/tmp/slugarch-type2-transport-src/CXLMemSim/tests/test_slugarch_type2_protocol.cpp`
- Modify: `/tmp/slugarch-type2-transport-src/CXLMemSim/CMakeLists.txt`

- [ ] **Step 1: Write the failing CRC and golden-HELLO tests**

Use this exact golden frame:

```cpp
static constexpr std::array<uint8_t, 72> kHelloGolden = {
    0x53,0x4c,0x54,0x32,0x01,0x00,0x01,0x00,
    0x48,0x00,0x00,0x00,0x00,0x00,0x00,0x00,
    0x01,0x00,0x00,0x00,0x00,0x00,0x00,0x00,
    0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,
    0xcf,0x70,0x20,0xd2,0x00,0x00,0x00,0x00,
    0x01,0x00,0x00,0x00,0x00,0x00,0x00,0x00,
    0x00,0x01,0x02,0x03,0x04,0x05,0x06,0x07,
    0x08,0x09,0x0a,0x0b,0x0c,0x0d,0x0e,0x0f,
    0x80,0x00,0x00,0x00,0x00,0x00,0x00,0x00,
};

static void testCrc32cIscsiVector() {
    constexpr std::array<uint8_t, 9> input = {'1','2','3','4','5','6','7','8','9'};
    REQUIRE(slugarch::type2::crc32c(input) == 0xe3069283U);
}

static void testEncodesNormativeHello() {
    slugarch::type2::Hello hello{};
    hello.header.request_id = 1;
    hello.role = slugarch::type2::Role::Qemu;
    for (size_t i = 0; i < hello.client_nonce.size(); ++i) {
        hello.client_nonce[i] = static_cast<uint8_t>(i);
    }
    auto frame = slugarch::type2::encodeHello(hello);
    REQUIRE(frame.length == kHelloGolden.size());
    REQUIRE(std::equal(kHelloGolden.begin(), kHelloGolden.end(),
                       frame.bytes.begin()));
}
```

Use the repository's existing standalone-test style: define a `REQUIRE(condition)`
macro that prints the failed expression and exits nonzero, call every test
function from `main()`, and add no external test-framework dependency. Also
add tests that flip byte 41 and require `DecodeError::Checksum`, set header
reserved byte 36 and require `DecodeError::Reserved`, set length 129 and
require `DecodeError::Length`, and set version 2 and require
`DecodeError::Version`.

- [ ] **Step 2: Register and run the test before implementation**

Add this CMake target:

```cmake
add_executable(test_slugarch_type2_protocol
    tests/test_slugarch_type2_protocol.cpp
    src/slugarch_type2_protocol.cpp)
target_include_directories(test_slugarch_type2_protocol PRIVATE include)
add_test(NAME test_slugarch_type2_protocol COMMAND test_slugarch_type2_protocol)
```

Run:

```bash
cmake -S /tmp/slugarch-type2-transport-src/CXLMemSim -B /tmp/slugarch-type2-transport-build/server -DCMAKE_BUILD_TYPE=Debug
cmake --build /tmp/slugarch-type2-transport-build/server --target test_slugarch_type2_protocol -j
```

Expected: compilation fails because `slugarch_type2_protocol.h` and its functions do not yet exist.

- [ ] **Step 3: Define exact wire types and validation results**

The public header must define these values and no packed native-wire structs:

```cpp
namespace slugarch::type2 {

inline constexpr std::array<uint8_t, 4> kMagic = {'S', 'L', 'T', '2'};
inline constexpr uint16_t kVersion = 1;
inline constexpr uint32_t kHeaderBytes = 40;
inline constexpr uint32_t kMaximumFrame = 128;
inline constexpr uint64_t kRequestTimeoutNs = 5'000'000'000ULL;

enum class FrameType : uint16_t {
    Hello = 1, Ack = 2, Read = 3, Write = 4,
    MemoryResponse = 5, CounterSnapshot = 6,
    CounterResponse = 7, Error = 255,
};
enum class Role : uint32_t { Qemu = 1, Oracle = 2 };
enum class Status : uint32_t {
    Success = 0, Protocol = 1, Range = 2,
    Checksum = 3, Backend = 4, Unsupported = 5,
};
enum class DecodeError {
    None, Short, Magic, Version, Type, Length, Flags, RequestId,
    ClientId, Checksum, Reserved, Body,
};
enum class IoResult { Ok, Timeout, Eof, SystemError, ProtocolError };

struct Header {
    FrameType type{};
    uint32_t frame_length{};
    uint64_t request_id{};
    uint64_t client_id{};
};
struct Hello {
    Header header{};
    Role role{};
    std::array<uint8_t, 16> client_nonce{};
};
struct Ack {
    Header header{};
    Status status{};
    std::array<uint8_t, 16> server_uuid{};
    uint64_t capacity{};
    uint64_t configured_base_latency{};
};
struct MemoryRequest {
    Header header{};
    uint32_t length{};
    uint64_t dpa{};
    uint64_t client_monotonic_ns{};
    std::array<uint8_t, 64> data{};
};
struct MemoryResponse {
    Header header{};
    Status status{};
    uint32_t returned_length{};
    uint64_t server_sequence{};
    uint64_t modeled_latency{};
    std::array<uint8_t, 64> data{};
};
struct CounterSnapshot {
    Header header{};
    uint64_t target_client_id{};
};
struct CounterResponse {
    Header header{};
    Status status{};
    uint64_t target_client_id{};
    uint64_t completed_reads{};
    uint64_t completed_writes{};
    uint64_t read_bytes{};
    uint64_t written_bytes{};
    uint64_t failed_requests{};
    uint64_t in_flight_requests{};
    uint64_t modeled_latency_sum{};
};
struct ErrorResponse {
    Header header{};
    Status status{};
    uint32_t reason{};
    uint64_t related_request_id{};
};
struct Frame {
    std::array<uint8_t, kMaximumFrame> bytes{};
    uint32_t length{};
};

uint32_t crc32c(std::span<const uint8_t> bytes);
Frame encodeHello(const Hello &hello);
Frame encodeAck(const Ack &ack);
Frame encodeMemoryRequest(FrameType type, const MemoryRequest &request);
Frame encodeMemoryResponse(const MemoryResponse &response);
Frame encodeCounterSnapshot(const CounterSnapshot &request);
Frame encodeCounterResponse(const CounterResponse &response);
Frame encodeError(const ErrorResponse &response);
DecodeError decodeHeader(std::span<const uint8_t> bytes, Header *header);
DecodeError decodeHello(std::span<const uint8_t> bytes, Hello *hello);
DecodeError decodeMemoryRequest(std::span<const uint8_t> bytes,
                                MemoryRequest *request);
DecodeError decodeMemoryResponse(std::span<const uint8_t> bytes,
                                 MemoryResponse *response);
DecodeError decodeCounterSnapshot(std::span<const uint8_t> bytes,
                                  CounterSnapshot *request);
IoResult readFrameUntil(int fd, uint64_t deadline_ns, Frame *frame);
IoResult writeFrameUntil(int fd, uint64_t deadline_ns, const Frame &frame);
uint64_t monotonicNowNs();

}  // namespace slugarch::type2
```

Use explicit `loadLe16/loadLe32/loadLe64` and `storeLe16/storeLe32/storeLe64` helpers. Encode with CRC bytes 32–35 zero, calculate CRC across `frame.length`, then store the CRC at byte 32.

- [ ] **Step 4: Implement fixed-length and exact-deadline rules**

The decoder must map types to these exact total lengths:

```cpp
static constexpr uint32_t expectedLength(FrameType type) {
    switch (type) {
    case FrameType::Hello: return 72;
    case FrameType::Ack: return 88;
    case FrameType::Read:
    case FrameType::Write:
    case FrameType::MemoryResponse: return 128;
    case FrameType::CounterSnapshot: return 48;
    case FrameType::CounterResponse: return 112;
    case FrameType::Error: return 56;
    }
    return 0;
}
```

The I/O loop must use one absolute deadline:

```cpp
static IoResult waitFd(int fd, short events, uint64_t deadline_ns) {
    for (;;) {
        uint64_t now = monotonicNowNs();
        if (now >= deadline_ns) {
            return IoResult::Timeout;
        }
        uint64_t remaining = deadline_ns - now;
        timespec timeout{
            .tv_sec = static_cast<time_t>(remaining / 1'000'000'000ULL),
            .tv_nsec = static_cast<long>(remaining % 1'000'000'000ULL),
        };
        pollfd pfd{.fd = fd, .events = events, .revents = 0};
        int rc = ppoll(&pfd, 1, &timeout, nullptr);
        if (rc > 0) return IoResult::Ok;
        if (rc == 0) return IoResult::Timeout;
        if (errno != EINTR) return IoResult::SystemError;
    }
}
```

`readFrameUntil()` first reads exactly 40 bytes, validates magic/version/type/length bounds, then reads exactly `frame_length - 40` bytes under the same deadline. A zero-byte receive before completion returns `IoResult::Eof`. `EINTR` retries without extending the deadline.

- [ ] **Step 5: Add fragmented-I/O, partial-EOF, and timeout tests**

Use `socketpair(AF_UNIX, SOCK_STREAM | SOCK_NONBLOCK, 0, sockets)`:

- Writer sends `kHelloGolden` one byte at a time; reader must return `Ok`.
- Writer sends 20 bytes and closes; reader must return `Eof`.
- Writer sends nothing with a 20 ms deadline; reader must return `Timeout`.
- Reader drains one byte at a time while writer sends a 128-byte frame; writer must return `Ok`.

Run:

```bash
cmake --build /tmp/slugarch-type2-transport-build/server --target test_slugarch_type2_protocol -j
ctest --test-dir /tmp/slugarch-type2-transport-build/server -R '^test_slugarch_type2_protocol$' --output-on-failure
```

Expected: all protocol tests pass.

- [ ] **Step 6: Commit only the codec**

Run:

```bash
git -C /tmp/slugarch-type2-transport-src/CXLMemSim add CMakeLists.txt include/slugarch_type2_protocol.h src/slugarch_type2_protocol.cpp tests/test_slugarch_type2_protocol.cpp
git -C /tmp/slugarch-type2-transport-src/CXLMemSim commit -m "protocol: add versioned SlugArch Type-2 codec"
```

Expected: one commit containing only the four listed paths.

---

### Task 3: Make CXLMemSim backing capacity exact and non-wrapping at the protocol boundary

**Files:**

- Modify: `/tmp/slugarch-type2-transport-src/CXLMemSim/include/shared_memory_manager.h`
- Modify: `/tmp/slugarch-type2-transport-src/CXLMemSim/src/shared_memory_manager.cc`
- Create: `/tmp/slugarch-type2-transport-src/CXLMemSim/tests/test_slugarch_type2_server.cpp`
- Modify: `/tmp/slugarch-type2-transport-src/CXLMemSim/CMakeLists.txt`

- [ ] **Step 1: Write the failing exact-capacity regression test**

```cpp
static void testFullConfiguredDataCapacity() {
    const std::string name = "/slugarch_t2_capacity_test_" +
                             std::to_string(::getpid());
    SharedMemoryManager memory(256, name);
    REQUIRE(memory.initialize());
    auto info = memory.get_shm_info();
    REQUIRE(info.data_size == 256ULL * 1024ULL * 1024ULL);
    REQUIRE(info.mapping_size > info.data_size);

    const uint8_t written = 0xa5;
    uint8_t read = 0;
    REQUIRE(memory.write_cacheline(info.data_size - 1, &written, 1));
    REQUIRE(memory.read_cacheline(info.data_size - 1, &read, 1));
    REQUIRE(read == written);
    shm_unlink(name.c_str());
}
```

Give this file the same standalone `REQUIRE` macro and `main()` pattern as
Task 2. Register it without GTest:

```cmake
add_executable(test_slugarch_type2_server
    tests/test_slugarch_type2_server.cpp)
target_include_directories(test_slugarch_type2_server PRIVATE include src)
target_link_libraries(test_slugarch_type2_server PRIVATE
    cxlmemsim_server_lib cxlmemsim ${RT_LIB} ${ATOMIC_LIB})
add_test(NAME test_slugarch_type2_server COMMAND test_slugarch_type2_server)
```

Run:

```bash
cmake --build /tmp/slugarch-type2-transport-build/server --target test_slugarch_type2_server -j
```

Expected: compilation fails because `SharedMemoryInfo` has no `data_size` or `mapping_size`; the current implementation would otherwise expose 56 bytes less than 256 MiB.

- [ ] **Step 2: Separate logical data bytes from mapping bytes**

Add these fields:

```cpp
struct SharedMemoryInfo {
    std::string shm_name;
    size_t size;          // compatibility alias for data_size
    size_t data_size;     // logical CXL bytes
    size_t mapping_size;  // header plus data bytes
    uint64_t base_addr;
    size_t num_cachelines;
};
```

In all non-SSD constructors, calculate:

```cpp
logical_capacity_bytes = capacity_mb * 1024ULL * 1024ULL;
shm_size = sizeof(SharedMemoryHeader) + logical_capacity_bytes;
```

For SSD mode, keep the header in `header_storage` and set `logical_capacity_bytes = ssd_config.capacity_bytes`.

Return:

```cpp
info.size = logical_capacity_bytes;
info.data_size = logical_capacity_bytes;
info.mapping_size = backing_mode == BackingMode::SsdStream
                        ? 0
                        : shm_size;
info.num_cachelines = logical_capacity_bytes / SHM_CACHELINE_SIZE;
```

Keep base-zero modulo behavior for legacy callers, but the new Type-2 service must perform strict `dpa <= capacity && length <= capacity - dpa` validation before calling this class.

- [ ] **Step 3: Preserve old-format isolation**

Increment the internal shared-memory `FORMAT_VERSION` from `1` to `2`. In protocol mode, require a unique name supplied by `--slugarch-shm-name`; do not silently reopen `/cxlmemsim_shared`.

Track whether `shm_open(..., O_CREAT | O_EXCL, ...)` created the object. Initialize and zero data only for a new object. If a protocol-mode object already exists, server startup must fail instead of preserving stale bytes.

- [ ] **Step 4: Run exact-capacity and legacy unit tests**

Run:

```bash
cmake --build /tmp/slugarch-type2-transport-build/server -j
ctest --test-dir /tmp/slugarch-type2-transport-build/server --output-on-failure
```

Expected: the new last-byte test passes and all pre-existing registered CTests pass.

- [ ] **Step 5: Commit the capacity correction separately**

Run:

```bash
git -C /tmp/slugarch-type2-transport-src/CXLMemSim add include/shared_memory_manager.h src/shared_memory_manager.cc tests/test_slugarch_type2_server.cpp CMakeLists.txt
git -C /tmp/slugarch-type2-transport-src/CXLMemSim commit -m "server: make CXL backing capacity exact"
```

Expected: no protocol-service implementation is included in this commit.

---

### Task 4: Add the default-off synchronous server service, counters, and gzip evidence

**Files:**

- Create: `/tmp/slugarch-type2-transport-src/CXLMemSim/include/slugarch_type2_server.h`
- Create: `/tmp/slugarch-type2-transport-src/CXLMemSim/src/slugarch_type2_server.cpp`
- Modify: `/tmp/slugarch-type2-transport-src/CXLMemSim/src/main_server.cc`
- Modify: `/tmp/slugarch-type2-transport-src/CXLMemSim/tests/test_slugarch_type2_server.cpp`
- Modify: `/tmp/slugarch-type2-transport-src/CXLMemSim/CMakeLists.txt`

- [ ] **Step 1: Write failing authority, role, range, and counter tests**

Extend the standalone test executable with a `ServerHarness` around a 256 MiB
test backing and two `socketpair` clients. Test functions must assert:

```cpp
static void testWriteThenReadUsesServerBytes(ServerHarness &h) {
    uint64_t qemu_id = h.handshake(Role::Qemu);
    std::array<uint8_t, 64> payload{};
    for (size_t i = 0; i < payload.size(); ++i) {
        payload[i] = static_cast<uint8_t>(0x5aU ^ i);
    }
    REQUIRE(h.write(qemu_id, 80ULL * MiB, payload) == Status::Success);
    REQUIRE(h.read(qemu_id, 80ULL * MiB, payload.size()) == payload);
}

static void testRejectsLastByteOverflowWithoutWrapping(ServerHarness &h) {
    uint64_t qemu_id = h.handshake(Role::Qemu);
    REQUIRE(h.readStatus(qemu_id, 256ULL * MiB - 32, 64) == Status::Range);
    REQUIRE(h.readStatus(qemu_id, 256ULL * MiB, 1) == Status::Range);
}

static void testCounterSnapshotIsOracleOnly(ServerHarness &h) {
    uint64_t qemu_id = h.handshake(Role::Qemu);
    REQUIRE(h.snapshotFrom(qemu_id, qemu_id).status == Status::Unsupported);
    uint64_t oracle_id = h.handshake(Role::Oracle);
    REQUIRE(h.snapshotFrom(oracle_id, qemu_id).status == Status::Success);
}
```

Also test:

- HELLO request ID is 1 and first post-handshake ID is greater than 1.
- Reused or decreasing IDs get `PROTOCOL` and disconnect.
- QEMU reads/writes are counted only under the QEMU client.
- Oracle reads/writes remain under the oracle client.
- `COUNTER_SNAPSHOT` does not alter any counter.
- A failed range request increments only `failed_requests`.
- `in_flight_requests` returns to zero after success and backend failure.
- The configured 400 ns base produces `modeled_latency=400` for both v1 READ and WRITE.
- once a QEMU client can read a successful response, a concurrent oracle
  snapshot already observes the matching completion count and byte total.

- [ ] **Step 2: Run the tests and verify the service is absent**

Run:

```bash
cmake --build /tmp/slugarch-type2-transport-build/server --target test_slugarch_type2_server -j
```

Expected: compilation fails because `SlugArchType2Server` is undefined.

- [ ] **Step 3: Define service state and counter ownership**

Use this interface:

```cpp
struct SlugArchType2Counters {
    Role role{};
    std::atomic<uint64_t> completed_reads{0};
    std::atomic<uint64_t> completed_writes{0};
    std::atomic<uint64_t> read_bytes{0};
    std::atomic<uint64_t> written_bytes{0};
    std::atomic<uint64_t> failed_requests{0};
    std::atomic<uint64_t> in_flight_requests{0};
    std::atomic<uint64_t> modeled_latency_sum{0};
};

class SlugArchType2Server {
public:
    SlugArchType2Server(SharedMemoryManager &memory,
                        uint64_t capacity_bytes,
                        uint64_t configured_base_latency_ns,
                        const std::string &gzip_event_path);
    void serveConnection(int fd);
    std::array<uint8_t, 16> serverUuid() const;

private:
    std::shared_ptr<SlugArchType2Counters> lookupClient(uint64_t id);
    Status handleMemory(uint64_t client_id, FrameType type,
                        const MemoryRequest &request,
                        MemoryResponse *response);
    Status handleSnapshot(Role requester_role,
                          const CounterSnapshot &request,
                          CounterResponse *response);
    void writeCompletionEvent(const CompletionEvent &event);
    void writeErrorEvent(const ErrorEvent &event);

    SharedMemoryManager &memory_;
    const uint64_t capacity_bytes_;
    const uint64_t configured_base_latency_ns_;
    const std::array<uint8_t, 16> server_uuid_;
    std::atomic<uint64_t> next_client_id_{1};
    std::atomic<uint64_t> next_server_sequence_{1};
    std::mutex clients_mutex_;
    std::unordered_map<uint64_t,
        std::shared_ptr<SlugArchType2Counters>> clients_;
    std::mutex evidence_mutex_;
    gzFile evidence_{nullptr};
};
```

Generate the server UUID once with `getrandom()` and fail startup if 16 bytes cannot be obtained. Client IDs and server sequence numbers must never be zero.

- [ ] **Step 4: Implement memory completion accounting with an in-flight guard**

Use strict subtraction-safe range validation:

```cpp
if (request.length == 0 || request.length > 64 ||
    request.dpa > capacity_bytes_ ||
    request.length > capacity_bytes_ - request.dpa) {
    counters->failed_requests.fetch_add(1, std::memory_order_relaxed);
    return Status::Range;
}
```

Use an RAII guard that increments `in_flight_requests` before backend access and decrements on every return. For a successful READ:

1. Read server bytes.
2. Assign one server sequence.
3. Set returned length and modeled latency.
4. Increment completed reads, read bytes, and modeled-latency sum.
5. Emit and flush one completion event.
6. Send the response.

For WRITE, commit server bytes before step 2. Completion accounting and its
flushed evidence must become visible before the response can become visible to
QEMU, so an immediate phase-end oracle snapshot cannot race the server
accounting. If sending the response then fails, retain the backend-completion
count, emit a delivery-error event, and mark the connection/boot invalid; the
QEMU/server mismatch makes that boot fail validation.

- [ ] **Step 5: Implement gzip JSONL evidence**

Add:

```cmake
find_package(OpenSSL REQUIRED COMPONENTS Crypto)
find_package(ZLIB REQUIRED)
```

Link `OpenSSL::Crypto` and `ZLIB::ZLIB` to `cxlmemsim_server_lib`, the server, and the server test.

Each completion line must contain exactly these keys:

```json
{"event":"completion","server_instance_id":"32hex","client_role":"qemu","client_id":1,"request_id":2,"server_sequence":1,"operation":"write","dpa":83886080,"length":64,"payload_sha256":"64hex","status":0,"configured_base_latency_ns":400,"modeled_latency_ns":400,"receive_monotonic_ns":1,"backend_complete_monotonic_ns":2}
```

Each error line must contain:

```json
{"event":"error","server_instance_id":"32hex","client_role":"qemu","client_id":1,"request_id":3,"server_sequence":0,"operation":"read","dpa":268435456,"length":1,"payload_sha256":"64hex","status":2,"reason":3,"receive_monotonic_ns":1,"error_monotonic_ns":2}
```

Use OpenSSL EVP SHA-256 over only the valid payload prefix: WRITE request bytes for writes and READ response bytes for reads. Protect `gzwrite()` and `gzflush(..., Z_SYNC_FLUSH)` with `evidence_mutex_`.

- [ ] **Step 6: Add explicit server CLI and keep the default legacy path**

Add these `ServerOptions` fields:

```cpp
bool slugarch_type2_protocol = false;
std::string slugarch_event_log;
std::string slugarch_shm_name;
```

Add options:

```text
--slugarch-type2-protocol[=true|false]
--slugarch-event-log <path>
--slugarch-shm-name <POSIX-shm-name>
```

When the mode is true, require:

- `comm_mode == TCP`
- `capacity == 256`
- nonempty event-log path
- a nonempty SHM name beginning with `/slugarch_type2_`

Construct one `SlugArchType2Server` and call `serveConnection(client_fd)` from `handle_client()`. When false, execute the unchanged legacy `ServerRequest` loop.

- [ ] **Step 7: Test legacy default and protocol mode**

Add one test that launches the server without `--slugarch-type2-protocol`, sends the existing 105-byte packed legacy request, and receives the existing 81-byte response. Add a second test that launches protocol mode and verifies a legacy 105-byte request is rejected within five seconds.

Run:

```bash
cmake --build /tmp/slugarch-type2-transport-build/server -j
ctest --test-dir /tmp/slugarch-type2-transport-build/server -R 'slugarch_type2|legacy' --output-on-failure
```

Expected: all new tests pass; legacy-default test passes unchanged.

- [ ] **Step 8: Commit the service**

Run:

```bash
git -C /tmp/slugarch-type2-transport-src/CXLMemSim add CMakeLists.txt include/slugarch_type2_server.h src/slugarch_type2_server.cpp src/main_server.cc tests/test_slugarch_type2_server.cpp
git -C /tmp/slugarch-type2-transport-src/CXLMemSim commit -m "server: add synchronous SlugArch Type-2 service"
```

Expected: the commit does not contain QEMU or guest harness changes.

---

### Task 5: Add the oracle client and server-only protocol smoke

**Files:**

- Create: `/tmp/slugarch-type2-transport-src/CXLMemSim/tools/slugarch_type2_oracle.cpp`
- Create: `/tmp/slugarch-type2-transport-src/CXLMemSim/qemu_integration/slugarch_type2_protocol_smoke.sh`
- Modify: `/tmp/slugarch-type2-transport-src/CXLMemSim/CMakeLists.txt`

- [ ] **Step 1: Write the smoke script before the oracle exists**

The script must:

1. Create a fresh `/tmp/slugarch-type2-protocol-smoke` and unique SHM name.
2. Start the new server at 400 ns and 256 MiB.
3. Start a test client with role QEMU, write and read 64 bytes at DPA 80 MiB, and print its assigned client ID.
4. Start an oracle connection and snapshot the QEMU client.
5. Require two QEMU completions, one read, one write, 64 read bytes, 64 written bytes, zero failures, and zero in flight.
6. Decompress the server log and require two QEMU completion events with distinct request IDs and server sequences.

Use these invocations:

```bash
"$SERVER" \
  --comm-mode=tcp \
  --port=10099 \
  --capacity=256 \
  --default_latency=400 \
  --slugarch-type2-protocol=true \
  --slugarch-event-log="$RUN/server-events.jsonl.gz" \
  --slugarch-shm-name="/slugarch_type2_protocol_smoke_$$" \
  >"$RUN/server.log" 2>&1 &
SERVER_PID=$!

"$ORACLE" --host 127.0.0.1 --port 10099 \
  --role qemu --write-pattern 83886080 64 0x534c5432 \
  --read-verify-pattern 83886080 64 0x534c5432 \
  --json "$RUN/qemu-client.json"

"$ORACLE" --host 127.0.0.1 --port 10099 \
  --role oracle --snapshot-client-file "$RUN/qemu-client.json" \
  --json "$RUN/counters.json"
```

- [ ] **Step 2: Run it and verify the expected missing-binary failure**

Run:

```bash
bash /tmp/slugarch-type2-transport-src/CXLMemSim/qemu_integration/slugarch_type2_protocol_smoke.sh
```

Expected: exit nonzero with `slugarch_type2_oracle not found`.

- [ ] **Step 3: Implement the oracle command contract**

The oracle must:

- use the shared codec and five-second exact-deadline I/O;
- use HELLO request ID 1 and monotonically increase IDs thereafter;
- default to `role=oracle`;
- allow `--role qemu` only for this server-only smoke;
- generate bytes with:

```cpp
static uint8_t patternByte(uint64_t seed, uint64_t index) {
    uint64_t x = seed + index * 0x9e3779b97f4a7c15ULL;
    x ^= x >> 30;
    x *= 0xbf58476d1ce4e5b9ULL;
    x ^= x >> 27;
    x *= 0x94d049bb133111ebULL;
    x ^= x >> 31;
    return static_cast<uint8_t>(x);
}
```

- print JSON containing role, client ID, server UUID, capacity, configured latency, operation results, request IDs, server sequences, and payload SHA-256;
- reject an ACK whose capacity is not 268,435,456, latency differs from the requested `--expect-latency`, maximum frame is not 128, status is not success, or client ID is zero.

- [ ] **Step 4: Build and pass the server-only smoke**

Run:

```bash
cmake --build /tmp/slugarch-type2-transport-build/server --target cxlmemsim_server slugarch_type2_oracle -j
SERVER=/tmp/slugarch-type2-transport-build/server/cxlmemsim_server ORACLE=/tmp/slugarch-type2-transport-build/server/slugarch_type2_oracle bash /tmp/slugarch-type2-transport-src/CXLMemSim/qemu_integration/slugarch_type2_protocol_smoke.sh
```

Expected: exit `0`; `counters.json` reports exactly one QEMU read, one QEMU write, 64 bytes each, zero failures, and zero in flight.

- [ ] **Step 5: Commit oracle and smoke**

Run:

```bash
git -C /tmp/slugarch-type2-transport-src/CXLMemSim add CMakeLists.txt tools/slugarch_type2_oracle.cpp qemu_integration/slugarch_type2_protocol_smoke.sh
git -C /tmp/slugarch-type2-transport-src/CXLMemSim commit -m "test: add SlugArch Type-2 oracle smoke"
```

---

### Task 6: Add the QEMU codec, deadline transport, and delay unit tests

**Files:**

- Create: `/tmp/slugarch-type2-transport-src/CXLMemSim/lib/qemu/include/hw/cxl/slugarch_type2_protocol.h`
- Create: `/tmp/slugarch-type2-transport-src/CXLMemSim/lib/qemu/hw/cxl/slugarch_type2_protocol.c`
- Create: `/tmp/slugarch-type2-transport-src/CXLMemSim/lib/qemu/tests/unit/test-cxl-type2-wire.c`
- Modify: `/tmp/slugarch-type2-transport-src/CXLMemSim/lib/qemu/hw/cxl/meson.build`
- Modify: `/tmp/slugarch-type2-transport-src/CXLMemSim/lib/qemu/tests/unit/meson.build`

- [ ] **Step 1: Add the same golden HELLO and CRC tests on the QEMU side**

Use the exact 72 bytes from Task 2 and assert:

```c
static void test_crc32c(void)
{
    static const uint8_t input[] = "123456789";
    g_assert_cmphex(slugarch_t2_crc32c(input, 9), ==, 0xe3069283U);
}

static void test_hello_golden(void)
{
    SlugArchT2Hello hello = {
        .request_id = 1,
        .role = SLUGARCH_T2_ROLE_QEMU,
    };
    SlugArchT2Frame frame = { 0 };
    for (size_t i = 0; i < sizeof(hello.client_nonce); i++) {
        hello.client_nonce[i] = i;
    }
    g_assert_true(slugarch_t2_encode_hello(&hello, &frame));
    g_assert_cmpuint(frame.length, ==, sizeof(k_hello_golden));
    g_assert_cmpmem(frame.bytes, frame.length,
                    k_hello_golden, sizeof(k_hello_golden));
}
```

Register tests for bad version, bad fixed length, nonzero reserved, bad CRC,
fragmented socket reads, partial EOF, and a 20 ms timeout. Add one total-budget
test whose peer consumes roughly 15 ms accepting the request and then stalls;
the response read must time out at the original 20 ms absolute deadline, not
20 ms after the send completes.

- [ ] **Step 2: Register and run before implementation**

Add the test source to `tests/unit/meson.build`, sourcing `hw/cxl/slugarch_type2_protocol.c` and depending on `qemuutil` plus `io`.

Run:

```bash
mkdir -p /tmp/slugarch-type2-transport-build/qemu
cd /tmp/slugarch-type2-transport-build/qemu
/tmp/slugarch-type2-transport-src/CXLMemSim/lib/qemu/configure --target-list=x86_64-softmmu --enable-debug
ninja tests/unit/test-cxl-type2-wire
```

Expected at this stage: configure or build fails because the new source/header
are absent. Do not configure or build the existing user checkout.

- [ ] **Step 3: Implement field-by-field wire encoding**

Define C structs containing host-order fields and a `uint8_t bytes[128]` frame. Do not cast the wire buffer to a native struct. Use QEMU `cpu_to_le16/32/64` helpers or explicit byte stores.

Implement CRC as:

```c
uint32_t slugarch_t2_crc32c(const uint8_t *data, size_t length)
{
    return crc32c(0xffffffffU, data, length);
}
```

Before verification, copy the received frame, zero bytes 32–35, and compare the stored little-endian CRC with the recalculated value.

- [ ] **Step 4: Implement nonblocking exact I/O and raw-clock delay**

Set the `QIOChannelSocket` nonblocking. Use `socket->fd`, `qemu_poll_ns()`, and a deadline from `CLOCK_MONOTONIC`.

The delay function must be:

```c
bool slugarch_t2_apply_delay(uint64_t requested_ns,
                             SlugArchT2DelayResult *result,
                             Error **errp)
{
    struct timespec start, now;
    uint64_t start_ns, now_ns;

    if (requested_ns > 1000000ULL) {
        error_setg(errp, "SlugArch Type-2 delay %" PRIu64
                   " exceeds 1000000 ns", requested_ns);
        return false;
    }
    if (clock_gettime(CLOCK_MONOTONIC_RAW, &start) != 0) {
        error_setg_errno(errp, errno, "CLOCK_MONOTONIC_RAW start failed");
        return false;
    }
    start_ns = (uint64_t)start.tv_sec * 1000000000ULL + start.tv_nsec;
    do {
        cpu_relax();
        if (clock_gettime(CLOCK_MONOTONIC_RAW, &now) != 0) {
            error_setg_errno(errp, errno, "CLOCK_MONOTONIC_RAW read failed");
            return false;
        }
        now_ns = (uint64_t)now.tv_sec * 1000000000ULL + now.tv_nsec;
    } while (now_ns - start_ns < requested_ns);

    result->requested_ns = requested_ns;
    result->actual_ns = now_ns - start_ns;
    result->undershot = result->actual_ns < requested_ns;
    result->overshoot_ns = result->actual_ns - requested_ns;
    return true;
}
```

Do not call or copy `cxl_memsim_inject_latency()` from `hw/mem/cxl_type3.c`; it subtracts IPC time and has different semantics.

- [ ] **Step 5: Configure and run an isolated QEMU unit build**

Run:

```bash
mkdir -p /tmp/slugarch-type2-transport-build/qemu
cd /tmp/slugarch-type2-transport-build/qemu
/tmp/slugarch-type2-transport-src/CXLMemSim/lib/qemu/configure --target-list=x86_64-softmmu --enable-debug
ninja tests/unit/test-cxl-type2-wire
./tests/unit/test-cxl-type2-wire --tap -k
```

Expected: all tests pass. The 80 ns delay test must assert `actual_ns >= 80`; the 1,000,001 ns test must fail with the exact maximum-delay message.

- [ ] **Step 6: Commit the QEMU protocol**

Run:

```bash
git -C /tmp/slugarch-type2-transport-src/CXLMemSim/lib/qemu add include/hw/cxl/slugarch_type2_protocol.h hw/cxl/slugarch_type2_protocol.c hw/cxl/meson.build tests/unit/test-cxl-type2-wire.c tests/unit/meson.build
git -C /tmp/slugarch-type2-transport-src/CXLMemSim/lib/qemu commit -m "cxl/type2: add SlugArch synchronous wire protocol"
```

---

### Task 7: Make QEMU Type-2 CFMWS accesses synchronous and server-authoritative

**Files:**

- Modify: `/tmp/slugarch-type2-transport-src/CXLMemSim/lib/qemu/include/hw/cxl/cxl_type2.h`
- Modify: `/tmp/slugarch-type2-transport-src/CXLMemSim/lib/qemu/hw/cxl/cxl_type2.c`
- Modify: `/tmp/slugarch-type2-transport-src/CXLMemSim/lib/qemu/tests/qtest/cxl-test.c`

- [ ] **Step 1: Write failing property and handshake qtests**

Add a Type-2 command line with:

```c
#define QEMU_T2_SYNC \
    "-machine q35,cxl=on " \
    "-device pxb-cxl,id=cxl.0,bus=pcie.0,bus_nr=52 " \
    "-M cxl-fmw.0.targets.0=cxl.0,cxl-fmw.0.size=256M " \
    "-device cxl-rp,id=rp0,bus=cxl.0,chassis=0,slot=0 " \
    "-device cxl-type2,id=t2,bus=rp0,gpu-mode=0," \
    "coherency-enabled=false,cache-size=128M,mem-size=256M," \
    "sync-type2-wire=on,type2-wire-version=1," \
    "slugarch-event-log=/tmp/qtest-slugarch-type2.jsonl," \
    "cxlmemsim-addr=127.0.0.1,cxlmemsim-port=10099 "
```

The qtest fixture substitutes a fresh `g_dir_make_tmp()` event path for the
illustrative `/tmp/qtest-slugarch-type2.jsonl` path, starts a small `SLT2`
fake server on an ephemeral loopback port, returns a valid ACK, and requires
QEMU to send the golden HELLO shape. Add negative cases for ACK capacity
64 MiB, configured latency 1,000,001 ns, wrong request ID, and bad CRC.

- [ ] **Step 2: Run and verify the unknown-property failure**

Run:

```bash
ninja -C /tmp/slugarch-type2-transport-build/qemu tests/qtest/cxl-test
env QTEST_QEMU_BINARY=/tmp/slugarch-type2-transport-build/qemu/qemu-system-x86_64 /tmp/slugarch-type2-transport-build/qemu/tests/qtest/cxl-test --tap -k -p /x86_64/pci/cxl/type2_sync_handshake
```

Expected: QEMU rejects `sync-type2-wire` as an unknown property.

- [ ] **Step 3: Add synchronous state and path counters**

Add:

```c
typedef struct CXLType2SlugArchState {
    bool enabled;
    bool connection_failed;
    bool shadow_after_write;
    uint16_t wire_version;
    char *event_log_path;
    char *phase_id;
    uint64_t next_request_id;
    uint64_t client_id;
    uint8_t server_uuid[16];
    uint64_t server_capacity;
    uint64_t configured_base_latency;
    uint64_t completed_reads;
    uint64_t completed_writes;
    uint64_t read_bytes;
    uint64_t written_bytes;
    uint64_t failed_requests;
    uint64_t timed_out_requests;
    uint64_t partial_io_failures;
    uint64_t mismatched_responses;
    uint64_t direct_cfmws_completions;
    uint64_t bar4_overlay_completions;
    uint64_t bulk_overlay_completions;
    uint64_t coherent_pool_completions;
    uint64_t local_shadow_completions;
    uint64_t local_cache_completions;
    uint64_t delay_events;
    uint64_t delay_undershoots;
} CXLType2SlugArchState;
```

Add:

```c
DEFINE_PROP_BOOL("sync-type2-wire", CXLType2State,
                 slugarch.enabled, false),
DEFINE_PROP_UINT16("type2-wire-version", CXLType2State,
                   slugarch.wire_version, 1),
DEFINE_PROP_STRING("slugarch-event-log", CXLType2State,
                   slugarch.event_log_path),
DEFINE_PROP_BOOL("slugarch-shadow-after-write", CXLType2State,
                 slugarch.shadow_after_write, false),
```

- [ ] **Step 4: Handshake during realization and fail closed**

When synchronous mode is on:

- reject SHM/PGAS transport;
- require `type2-wire-version == 1` and a nonempty absolute
  `slugarch-event-log` path;
- require `device_mem_size == 256 * MiB`;
- require a successful TCP connection;
- send QEMU HELLO request ID 1;
- require ACK request ID 1, nonzero client ID, version 1, max frame 128, success, capacity 256 MiB, and configured latency no greater than 1 ms;
- set `next_request_id = 2`;
- do not start `cxlmemsim_recv_thread`.

Any failure must `error_setg(errp, "SlugArch Type-2 handshake failed: ...")`, close the socket, and fail device realization. Legacy mode retains its warning-and-continue behavior.

- [ ] **Step 5: Implement one serialized request/response transaction**

Add:

```c
static MemTxResult cxl_type2_slugarch_access(CXLType2State *ct2d,
                                             bool is_write,
                                             hwaddr dpa,
                                             uint64_t *value,
                                             unsigned size,
                                             MemTxAttrs attrs);

MemTxResult cxl_type2_cfmws_read(PCIDevice *pdev, hwaddr dpa,
                                 uint64_t *value, unsigned size,
                                 MemTxAttrs attrs);
MemTxResult cxl_type2_cfmws_write(PCIDevice *pdev, hwaddr dpa,
                                  uint64_t value, unsigned size,
                                  MemTxAttrs attrs);
```

Under `memsim.lock`:

1. Reject a failed connection and subtraction-unsafe range.
2. Encode READ or WRITE with the next request ID and `CLOCK_MONOTONIC` timestamp.
3. Use `stn_le_p()` for write data.
4. Compute one absolute five-second `CLOCK_MONOTONIC` deadline before the first
   request byte, then pass that unchanged deadline through both send and
   response receive.
5. Require `MEMORY_RESPONSE`, matching request ID/client ID, success, matching returned length, nonzero server sequence, and latency at most 1 ms.
6. For READ, use `ldn_le_p()` to return server bytes.
7. Apply the returned delay.
8. Update optional local shadow only after a successful write.
9. Increment direct/success counters.
10. Emit one structured delay event.

On any failure, mark `connection_failed=true`, increment the exact failure counter, close the socket, and return `MEMTX_ERROR`. There is no reconnect inside one boot.

- [ ] **Step 6: Emit joinable QEMU events**

Open `slugarch-event-log` with create-exclusive semantics during realization;
never append to a pre-existing file. Emit one JSONL completion line per
successful response:

```json
{"event":"completion","client_id":1,"request_id":2,"server_sequence":1,"operation":"read","dpa":83886080,"length":8,"payload_sha256":"64hex","status":0,"returned_modeled_latency_ns":400,"requested_delay_ns":400,"applied_delay_ns":463,"delay_overshoot_ns":63,"delay_undershot":false,"path":"direct_cfmws","phase_id":"sentinel_oracle_to_guest"}
```

Compute SHA-256 with `qcrypto_hash_bytes(QCRYPTO_HASH_ALGO_SHA256, ...)`.
Emit one handshake JSONL line containing client ID, server UUID, capacity,
configured latency, and protocol version. After each completion and at device
exit, emit:

```json
{"event":"path_counters","phase_id":"sentinel_oracle_to_guest","direct_cfmws":1,"bar4_overlay":0,"local_shadow":0,"local_cache":0,"bulk_overlay":0,"coherent_pool":0}
```

Flush after every line. A write or flush failure marks the connection failed
and returns `MEMTX_ERROR`.

- [ ] **Step 7: Add phase and counter QMP/QOM observability**

Register writable string property `slugarch-phase-id`. Its setter accepts only
`[A-Za-z0-9_.:-]{1,96}`, duplicates the value under `memsim.lock`, and
defaults to `idle`. Every completion snapshots the current value into
`phase_id`.

Register read-only QOM properties for client ID, completed reads/writes,
read/written bytes, every failure counter, direct-CFMWS completions,
aggregate BAR4-overlay completions, bulk/coherent overlay completions,
local-shadow/cache completions, delay events, and delay undershoots. The
harness uses generic QMP `qom-set` before each phase and `qom-get` at phase
boundaries.

Use these exact property names:

```text
slugarch-client-id
slugarch-completed-reads
slugarch-completed-writes
slugarch-read-bytes
slugarch-written-bytes
slugarch-failed-requests
slugarch-timed-out-requests
slugarch-partial-io-failures
slugarch-mismatched-responses
slugarch-direct-cfmws
slugarch-bar4-overlay
slugarch-bulk-overlay
slugarch-coherent-pool
slugarch-local-shadow
slugarch-local-cache
slugarch-delay-events
slugarch-delay-undershoots
```

Add qtests that set `slugarch-phase-id` to `phase:test`, reject a value with
whitespace, read each counter, and verify that an emitted completion carries
`"phase_id":"phase:test"`.

- [ ] **Step 8: Make overlay completions observable in experiment mode**

Priority-2 RAM regions bypass callbacks, so their completions cannot be counted. In synchronous mode only:

- retain the allocated RAM backing buffers;
- expose distinct priority-2 IO proxy regions for bulk staging and coherent
  pool;
- proxy reads/writes to the corresponding backing pointer;
- increment `bar4_overlay_completions` and either
  `bulk_overlay_completions` or `coherent_pool_completions` in the
  corresponding proxy operation.

In legacy mode, retain the existing direct priority-2 RAM subregions unchanged. Increment `local_cache_completions` only when a legacy cache lookup supplies a value. Never increment `local_shadow_completions` for a shadow update; that counter means an access was served locally.

- [ ] **Step 9: Pass handshake and negative qtests**

Run:

```bash
ninja -C /tmp/slugarch-type2-transport-build/qemu qemu-system-x86_64 tests/qtest/cxl-test
env QTEST_QEMU_BINARY=/tmp/slugarch-type2-transport-build/qemu/qemu-system-x86_64 /tmp/slugarch-type2-transport-build/qemu/tests/qtest/cxl-test --tap -k -p /x86_64/pci/cxl/type2_sync
```

Expected:

- valid ACK realizes the device;
- 64 MiB capacity, bad CRC, wrong request ID, and configured latency
  1,000,001 ns each fail realization;
- QMP phase set/get and all counter reads work;
- no receiver thread consumes a synchronous response.

- [ ] **Step 10: Commit synchronous Type-2 behavior**

Run:

```bash
git -C /tmp/slugarch-type2-transport-src/CXLMemSim/lib/qemu add include/hw/cxl/cxl_type2.h hw/cxl/cxl_type2.c tests/qtest/cxl-test.c
git -C /tmp/slugarch-type2-transport-src/CXLMemSim/lib/qemu commit -m "cxl/type2: make SlugArch memory server authoritative"
```

---

### Task 8: Correct the Type-2 DVSEC to one active 256 MiB range

**Files:**

- Modify: `/tmp/slugarch-type2-transport-src/CXLMemSim/lib/qemu/hw/cxl/cxl_type2.c`
- Modify: `/tmp/slugarch-type2-transport-src/CXLMemSim/lib/qemu/tests/qtest/cxl-test.c`

- [ ] **Step 1: Add failing DVSEC assertions**

The qtest must locate the CXL Device DVSEC and assert:

- Cache Capable, IO Capable, Mem Capable, Mem HW Init, and one HDM decoder are advertised.
- `cap2` has unit code 2 (1 MiB) and size field 128, which decodes to the
  configured 128 MiB cache capacity.
- Range 1 base is zero.
- Range 1 size is 256 MiB.
- Range 1 valid and active bits are both one.
- Range 2 size, base, valid, and active are all zero.

Run:

```bash
env QTEST_QEMU_BINARY=/tmp/slugarch-type2-transport-build/qemu/qemu-system-x86_64 /tmp/slugarch-type2-transport-build/qemu/tests/qtest/cxl-test --tap -k -p /x86_64/pci/cxl/type2_dvsec
```

Expected before the fix: range 1 incorrectly reports cache size and range 2 is not active.

- [ ] **Step 2: Encode the correct range**

Replace the range fields with:

```c
.cap2 = ((((ct2d->cache_size / MiB) & 0xff) << 8) | 2),
.range1_size_hi = ct2d->device_mem_size >> 32,
.range1_size_lo = (2U << 5) | (2U << 2) | 0x3U |
                  (ct2d->device_mem_size & 0xf0000000U),
.range1_base_hi = 0,
.range1_base_lo = 0,
.range2_size_hi = 0,
.range2_size_lo = 0,
.range2_base_hi = 0,
.range2_base_lo = 0,
```

Before building the DVSEC, require the cache size to be an integral 1 through
255 MiB value so `cap2` cannot truncate. The `0x3` bits mean valid and active.
Do not use `0xfffffff0`; Device DVSEC size low bits carry attributes and only
the upper size nibble belongs there.

- [ ] **Step 3: Run qtests and commit**

Run:

```bash
ninja -C /tmp/slugarch-type2-transport-build/qemu qemu-system-x86_64 tests/qtest/cxl-test
env QTEST_QEMU_BINARY=/tmp/slugarch-type2-transport-build/qemu/qemu-system-x86_64 /tmp/slugarch-type2-transport-build/qemu/tests/qtest/cxl-test --tap -k -p /x86_64/pci/cxl/type2_dvsec
git -C /tmp/slugarch-type2-transport-src/CXLMemSim/lib/qemu add hw/cxl/cxl_type2.c tests/qtest/cxl-test.c
git -C /tmp/slugarch-type2-transport-src/CXLMemSim/lib/qemu commit -m "cxl/type2: advertise one active memory range"
```

Expected: DVSEC qtest passes and the commit contains only DVSEC/test changes.

---

### Task 9: Route the exact one-target CFMWS shape to Type-2

**Files:**

- Create: `/tmp/slugarch-type2-transport-src/CXLMemSim/lib/qemu/tests/unit/test-cxl-type2-route.c`
- Modify: `/tmp/slugarch-type2-transport-src/CXLMemSim/lib/qemu/hw/cxl/cxl-host.c`
- Modify: `/tmp/slugarch-type2-transport-src/CXLMemSim/lib/qemu/include/hw/cxl/cxl_type2.h`
- Modify: `/tmp/slugarch-type2-transport-src/CXLMemSim/lib/qemu/tests/unit/meson.build`
- Modify: `/tmp/slugarch-type2-transport-src/CXLMemSim/lib/qemu/tests/qtest/cxl-test.c`

- [ ] **Step 1: Write a pure failing shape validator test**

Expose:

```c
bool cxl_type2_cfmws_shape_valid(unsigned total_windows,
                                 unsigned num_targets,
                                 uint8_t encoded_ways,
                                 uint64_t window_size,
                                 uint64_t dpa_base,
                                 uint64_t device_size,
                                 uint64_t access_offset,
                                 unsigned access_size);
```

Test one valid tuple:

```c
g_assert_true(cxl_type2_cfmws_shape_valid(
    1, 1, 0, 256 * MiB, 0, 256 * MiB, 80 * MiB, 8));
```

Test false for:

- total windows 2;
- targets 2;
- encoded ways 1;
- window 512 MiB;
- DPA base 4 KiB;
- device size 64 MiB;
- access offset exactly 256 MiB;
- `UINT64_MAX - 3` with access size 8.

- [ ] **Step 2: Run and verify the missing-validator failure**

Run:

```bash
ninja -C /tmp/slugarch-type2-transport-build/qemu tests/unit/test-cxl-type2-route
```

Expected: build fails with undefined `cxl_type2_cfmws_shape_valid`.

- [ ] **Step 3: Refactor route discovery without changing Type-3 behavior**

Replace the Type-3-only return assumption with:

```c
typedef enum CXLRouteType {
    CXL_ROUTE_NONE,
    CXL_ROUTE_TYPE2_DIRECT,
    CXL_ROUTE_TYPE3,
} CXLRouteType;

typedef struct CXLRouteResult {
    PCIDevice *device;
    CXLRouteType type;
    bool traversed_switch;
} CXLRouteResult;
```

At the device directly behind the root port:

- return `TYPE_CXL_TYPE3` as `CXL_ROUTE_TYPE3`;
- return `TYPE_CXL_TYPE2` as `CXL_ROUTE_TYPE2_DIRECT`;
- otherwise continue the existing one-switch Type-3 logic.

Never accept a Type-2 endpoint behind a switch.

- [ ] **Step 4: Validate the complete shape before Type-2 dispatch**

For a Type-2 result:

1. Count fixed windows with `cxl_fmws_get_all()` and require exactly one.
2. Require `fw->num_targets == 1`.
3. Require `fw->enc_int_ways == 0`.
4. Require `fw->size == 256 * MiB`.
5. Require the endpoint synchronous mode.
6. Require endpoint DPA base zero and `device_mem_size == 256 * MiB`.
7. Check `addr <= fw->size` and `size <= fw->size - addr`.
8. Call `cxl_type2_cfmws_read/write(d, addr, ...)` with the CFMWS-relative address.

Do not add `fw->base` for Type-2. Keep `addr + fw->base` for the existing Type-3 calls.

Unsupported Type-2 reads return `MEMTX_ERROR` with data zeroed. Unsupported Type-2 writes return `MEMTX_ERROR`, not the existing silent invalid-Type-3 write behavior.

- [ ] **Step 5: Add dispatch qtests**

The fake server test must issue one CFMWS read and one write after programming the host-bridge decoder. Assert:

- server sees DPA 80 MiB, not CFMWS HPA;
- read data comes from the fake server;
- write bytes reach the fake server;
- direct CFMWS count becomes two;
- a two-target topology returns `MEMTX_ERROR`;
- a 512 MiB window returns `MEMTX_ERROR`;
- a Type-2 endpoint behind a USP/DSP switch returns `MEMTX_ERROR`;
- existing Type-3 CFMWS qtests remain green.

- [ ] **Step 6: Run route tests and commit**

Run:

```bash
ninja -C /tmp/slugarch-type2-transport-build/qemu qemu-system-x86_64 tests/unit/test-cxl-type2-route tests/qtest/cxl-test
/tmp/slugarch-type2-transport-build/qemu/tests/unit/test-cxl-type2-route --tap -k
env QTEST_QEMU_BINARY=/tmp/slugarch-type2-transport-build/qemu/qemu-system-x86_64 /tmp/slugarch-type2-transport-build/qemu/tests/qtest/cxl-test --tap -k
```

Expected: route unit tests, all Type-2 qtests, and all existing CXL qtests pass.

Commit:

```bash
git -C /tmp/slugarch-type2-transport-src/CXLMemSim/lib/qemu add hw/cxl/cxl-host.c include/hw/cxl/cxl_type2.h tests/unit/test-cxl-type2-route.c tests/unit/meson.build tests/qtest/cxl-test.c
git -C /tmp/slugarch-type2-transport-src/CXLMemSim/lib/qemu commit -m "cxl/type2: route one-target CFMWS accesses"
```

---

### Task 10: Repair the guest-kernel QEMU DVSEC capacity and decoder path

**Files:**

- Create: `/tmp/slugarch-type2-transport-src/cxl/drivers/cxl/type2_dvsec_capacity.h`
- Create: `/tmp/slugarch-type2-transport-src/cxl/tools/testing/cxl/type2_dvsec_capacity_test.c`
- Modify: `/tmp/slugarch-type2-transport-src/cxl/drivers/cxl/cxl.h`
- Modify: `/tmp/slugarch-type2-transport-src/cxl/drivers/cxl/cxl_type2_accel.c`
- Modify: `/tmp/slugarch-type2-transport-src/cxl/drivers/cxl/core/hdm.c`

- [ ] **Step 1: Write the failing pure capacity tests**

Define the intended helper contract in the test:

```c
static void test_repeatable_qemu_capacity(void)
{
    unsigned long long capacity = 0;
    const unsigned long long size_256m = 256ULL * 1024ULL * 1024ULL;
    const unsigned long long size_512m = 512ULL * 1024ULL * 1024ULL;

    assert(cxl_type2_validate_dvsec_pair(size_256m, size_256m,
                                         size_256m, &capacity) == 0);
    assert(capacity == size_256m);
    assert(cxl_type2_validate_dvsec_pair(0, 0, size_256m,
                                         &capacity) == -ENODEV);
    assert(cxl_type2_validate_dvsec_pair(size_256m, size_512m,
                                         size_256m, &capacity) == -EIO);
    assert(cxl_type2_validate_dvsec_pair(size_512m, size_512m,
                                         size_256m, &capacity) == -EINVAL);
    assert(cxl_type2_qemu_compat(0x8086, 0x0d92, true));
    assert(!cxl_type2_qemu_compat(0x8086, 0x0d92, false));
    assert(!cxl_type2_qemu_compat(0x8086, 0x0d93, true));
    assert(!cxl_type2_qemu_compat(0x1e98, 0x0d92, true));
}
```

Compile the standalone helper before adding the header:

```bash
cc -O2 -Wall -Wextra -Werror -std=c11 \
  -o /tmp/type2_dvsec_capacity_test \
  /tmp/slugarch-type2-transport-src/cxl/tools/testing/cxl/type2_dvsec_capacity_test.c
```

Expected: compilation fails because `type2_dvsec_capacity.h` is missing.

- [ ] **Step 2: Implement the pure repeatability helper**

```c
static inline int cxl_type2_validate_dvsec_pair(
        unsigned long long first, unsigned long long second,
        unsigned long long expected, unsigned long long *capacity)
{
    if (!first || !second)
        return -ENODEV;
    if (first != second)
        return -EIO;
    if (expected && first != expected)
        return -EINVAL;
    *capacity = first;
    return 0;
}

static inline bool cxl_type2_qemu_compat(
        unsigned int vendor, unsigned int device, bool use_dvsec_hdm)
{
    return use_dvsec_hdm && vendor == 0x8086 && device == 0x0d92;
}
```

The header includes `<linux/errno.h>` and `<stdbool.h>` and otherwise uses only
C types accepted by both the kernel and a userspace compiler. Build and run:

```bash
cc -O2 -Wall -Wextra -Werror -std=c11 \
  -o /tmp/type2_dvsec_capacity_test \
  /tmp/slugarch-type2-transport-src/cxl/tools/testing/cxl/type2_dvsec_capacity_test.c
/tmp/type2_dvsec_capacity_test
```

Expected: exit `0`.

- [ ] **Step 3: Make QEMU capacity come from two active DVSEC snapshots**

For vendor/device `8086:0d92` with `use_dvsec_hdm=1`:

1. Read both Device DVSEC ranges.
2. Count only ranges with both valid and active bits.
3. Require exactly one active range.
4. Require its base to be zero.
5. Read the snapshot a second time.
6. Validate equal total capacity and exactly 256 MiB.
7. Set:

```c
mds->total_bytes = mapped_capacity;
mds->volatile_only_bytes = mapped_capacity;
mds->persistent_only_bytes = 0;
mds->active_volatile_bytes = mapped_capacity;
mds->active_persistent_bytes = 0;
mds->partition_align_bytes = SZ_256M;
strscpy(mds->firmware_version, "QEMU-DVSEC-v1",
        sizeof(mds->firmware_version));
mds->cxlds.media_ready = true;
mds->cxlds.qemu_type2_dvsec_compat = true;
```

Add `bool qemu_type2_dvsec_compat` to `struct cxl_dev_state`. It remains false
by default and is set only after the vendor/device guard and both DVSEC
snapshots pass. All non-QEMU devices retain the two-mailbox-Identify path and
never set this flag.

- [ ] **Step 4: Decouple zero-based DPA from endpoint HPA**

The current dirty `cxl_setup_hdm_decoder_from_dvsec()` copies the zero-based DVSEC range into `cxld->hpa_range` and locks it. That cannot match QEMU's high CFMWS HPA.

Only when `cxlds->dvsec_hdm_devmem` and
`cxlds->qemu_type2_dvsec_compat` are both true:

- reserve DPA `[0, mapped_capacity)`;
- set target type `CXL_DECODER_DEVMEM`;
- set `cxld->hpa_range = { .start = 0, .end = -1 }`;
- set `commit = cxl_decoder_commit_soft`;
- set `reset = cxl_decoder_reset_soft`;
- set endpoint state `CXL_DECODER_STATE_MANUAL`;
- do not set `CXL_DECODER_F_LOCK`.

The later `cxl create-region` flow then assigns the root-decoder HPA while the endpoint DPA remains zero-based.

Add a negative helper/unit case proving that another device with
`dvsec_hdm_devmem=true` but `qemu_type2_dvsec_compat=false` retains the normal
locked DVSEC-HDM decoder behavior.

- [ ] **Step 5: Build the helper, module, and kernel**

Run:

```bash
make -C /tmp/slugarch-type2-transport-src/cxl olddefconfig
make -C /tmp/slugarch-type2-transport-src/cxl -j"$(nproc)" bzImage modules
cc -O2 -Wall -Wextra -Werror -std=c11 \
  -o /tmp/type2_dvsec_capacity_test \
  /tmp/slugarch-type2-transport-src/cxl/tools/testing/cxl/type2_dvsec_capacity_test.c
/tmp/type2_dvsec_capacity_test
```

Expected:

- `arch/x86/boot/bzImage` exists;
- `drivers/cxl/cxl_type2_accel.ko` exists;
- the capacity helper test exits `0`;
- no unresolved symbol or modversion error appears.

- [ ] **Step 6: Commit only the QEMU Type-2 kernel compatibility**

Run:

```bash
git -C /tmp/slugarch-type2-transport-src/cxl add drivers/cxl/type2_dvsec_capacity.h drivers/cxl/cxl.h drivers/cxl/cxl_type2_accel.c drivers/cxl/core/hdm.c tools/testing/cxl/type2_dvsec_capacity_test.c
git -C /tmp/slugarch-type2-transport-src/cxl commit -m "cxl/type2: derive QEMU capacity from active DVSEC"
```

Expected: unrelated captured CapCXL/tmatmul changes remain unstaged in the isolated kernel clone.

---

### Task 11: Build a matching immutable guest kernel/module image boundary

**Files:**

- Create copied image: `/tmp/slugarch-type2-transport-build/guest/slugarch-type2.img`
- Create module root: `/tmp/slugarch-type2-transport-build/guest/module-root`
- Create config inside copied image: `/etc/modprobe.d/slugarch-type2.conf`

- [ ] **Step 1: Hash and copy the base image without modifying it**

Run:

```bash
mkdir -p /tmp/slugarch-type2-transport-build/guest
sha256sum /home/victoryang00/CXLMemSim/build/qemu1.img > /tmp/slugarch-type2-transport-build/guest/base-image.sha256
cp --reflink=auto --sparse=always /home/victoryang00/CXLMemSim/build/qemu1.img /tmp/slugarch-type2-transport-build/guest/slugarch-type2.img
```

Expected: the base-image hash still verifies after the copy.

- [ ] **Step 2: Install the complete matching module set into a staging root**

Run:

```bash
make -C /tmp/slugarch-type2-transport-src/cxl INSTALL_MOD_PATH=/tmp/slugarch-type2-transport-build/guest/module-root modules_install
KREL="$(make -s -C /tmp/slugarch-type2-transport-src/cxl kernelrelease)"
test -f "/tmp/slugarch-type2-transport-build/guest/module-root/lib/modules/$KREL/kernel/drivers/cxl/cxl_type2_accel.ko"
```

Expected: the full CXL core/module dependency set exists under one kernel release.

- [ ] **Step 3: Inject modules and module options into only the copied image**

Create `/tmp/slugarch-type2-transport-build/guest/slugarch-type2.conf`:

```text
options cxl_type2_accel enable_cache=0 enable_memdev=1 use_dvsec_hdm=1 allow_uncommitted_hdm=0
```

Run:

```bash
KREL="$(make -s -C /tmp/slugarch-type2-transport-src/cxl kernelrelease)"
guestfish --rw -a /tmp/slugarch-type2-transport-build/guest/slugarch-type2.img -m /dev/sda copy-in "/tmp/slugarch-type2-transport-build/guest/module-root/lib/modules/$KREL" /lib/modules
guestfish --rw -a /tmp/slugarch-type2-transport-build/guest/slugarch-type2.img -m /dev/sda copy-in /tmp/slugarch-type2-transport-build/guest/slugarch-type2.conf /etc/modprobe.d
```

Expected: `virt-ls` shows the new `cxl_type2_accel.ko` and configuration in the copied image; the base image hash remains unchanged.

- [ ] **Step 4: Record the inseparable kernel/image tuple**

Run:

```bash
KREL="$(make -s -C /tmp/slugarch-type2-transport-src/cxl kernelrelease)"
sha256sum /tmp/slugarch-type2-transport-src/cxl/arch/x86/boot/bzImage /tmp/slugarch-type2-transport-build/guest/slugarch-type2.img /tmp/slugarch-type2-transport-src/cxl/drivers/cxl/cxl_type2_accel.ko > /tmp/slugarch-type2-transport-build/guest/kernel-image-module.sha256
modinfo /tmp/slugarch-type2-transport-src/cxl/drivers/cxl/cxl_type2_accel.ko > /tmp/slugarch-type2-transport-build/guest/cxl_type2_accel.modinfo
grep -F "vermagic:" /tmp/slugarch-type2-transport-build/guest/cxl_type2_accel.modinfo | grep -F "$KREL"
```

Expected: `vermagic` contains the same `KREL`. The old March 2 module in the base image is not used as evidence.

---

### Task 12: Add no-guest negotiation smoke and prove legacy mismatch rejection

**Files:**

- Create: `/tmp/slugarch-type2-transport-src/CXLMemSim/qemu_integration/slugarch_type2_no_guest_smoke.sh`

- [ ] **Step 1: Write the smoke with three explicit cases**

Case A starts the new server and synchronous QEMU:

```bash
timeout 10 "$QEMU" \
  -accel tcg \
  -machine q35,cxl=on,cxl-fmw.0.targets.0=cxl.0,cxl-fmw.0.size=256M \
  -m 2G -smp 2 -nodefaults -display none -serial none -monitor none -S \
  -device pxb-cxl,id=cxl.0,bus=pcie.0,bus_nr=52 \
  -device cxl-rp,id=rp0,bus=cxl.0,chassis=0,slot=0 \
  -device cxl-type2,id=t2,bus=rp0,gpu-mode=0,coherency-enabled=false,cache-size=128M,mem-size=256M,sync-type2-wire=on,type2-wire-version=1,slugarch-event-log="$RUN/case-a-qemu-events.jsonl",cxlmemsim-addr=127.0.0.1,cxlmemsim-port=10100
```

Expected: timeout exit `124` after QEMU remains paused, with exactly one successful `SLUGARCH_T2_HANDSHAKE` line and zero server memory completions.

Case B starts the legacy server without the new flag and the same QEMU.

Use `$RUN/case-b-qemu-events.jsonl` for Case B so create-exclusive evidence
does not collide with Case A. Expected: QEMU exits nonzero within five seconds
with `SlugArch Type-2 handshake failed`; it must not reach `Device realized`.

Case C starts the new server and sends one legacy 105-byte request.

Expected: server emits one protocol error and closes the connection; completed memory counters remain zero.

- [ ] **Step 2: Run before building the new QEMU**

Run:

```bash
bash /tmp/slugarch-type2-transport-src/CXLMemSim/qemu_integration/slugarch_type2_no_guest_smoke.sh
```

Expected: script fails closed if `QEMU`, `SERVER`, or the protocol property is unavailable.

- [ ] **Step 3: Build and pass all three cases**

Run:

```bash
cmake --build /tmp/slugarch-type2-transport-build/server --target cxlmemsim_server slugarch_type2_oracle -j
ninja -C /tmp/slugarch-type2-transport-build/qemu qemu-system-x86_64
QEMU=/tmp/slugarch-type2-transport-build/qemu/qemu-system-x86_64 SERVER=/tmp/slugarch-type2-transport-build/server/cxlmemsim_server ORACLE=/tmp/slugarch-type2-transport-build/server/slugarch_type2_oracle bash /tmp/slugarch-type2-transport-src/CXLMemSim/qemu_integration/slugarch_type2_no_guest_smoke.sh
```

Expected: script exits `0` only after validating A, B, and C.

- [ ] **Step 4: Commit the no-guest smoke**

Run:

```bash
git -C /tmp/slugarch-type2-transport-src/CXLMemSim add qemu_integration/slugarch_type2_no_guest_smoke.sh
git -C /tmp/slugarch-type2-transport-src/CXLMemSim commit -m "test: prove Type-2 protocol negotiation"
```

---

### Task 13: Add exact guest region setup and a DAX sentinel helper

**Files:**

- Create: `/tmp/slugarch-type2-transport-src/CXLMemSim/qemu_integration/slugarch_type2_guest_setup.sh`
- Create: `/tmp/slugarch-type2-transport-src/CXLMemSim/qemu_integration/slugarch_type2_dax_sentinel.c`

- [ ] **Step 1: Write host-file tests for the DAX helper**

The helper CLI is:

```text
slugarch_type2_dax_sentinel --device PATH --offset BYTES --length 64 --mode read-verify|write --seed UINT64
```

Compile it, create a 128 MiB regular file, and test:

```bash
cc -O2 -Wall -Wextra -Werror -std=c11 -o /tmp/slugarch_type2_dax_sentinel /tmp/slugarch-type2-transport-src/CXLMemSim/qemu_integration/slugarch_type2_dax_sentinel.c
truncate -s 128M /tmp/slugarch_type2_dax_test.bin
/tmp/slugarch_type2_dax_sentinel --device /tmp/slugarch_type2_dax_test.bin --offset 83886080 --length 64 --mode write --seed 0x534c5432
/tmp/slugarch_type2_dax_sentinel --device /tmp/slugarch_type2_dax_test.bin --offset 83886080 --length 64 --mode read-verify --seed 0x534c5432
```

Expected after implementation: two JSON lines with `"status":"pass"` and identical SHA-256.

- [ ] **Step 2: Implement page-aligned shared DAX mapping**

The helper must:

- open with `O_RDWR | O_SYNC`;
- page-align the mapping offset;
- map only enough bytes to cover the 64-byte line;
- use `MAP_SHARED`;
- require an 8-byte-aligned work offset and copy the line exactly once through
  eight aligned `volatile uint64_t` loads for `read-verify` or eight aligned
  `volatile uint64_t` stores for `write`;
- hash and compare a local 64-byte buffer after the mapped access; never pass
  the DAX pointer to OpenSSL or `memcpy`, which could change transaction count;
- use the Task 5 deterministic pattern;
- execute `atomic_thread_fence(memory_order_seq_cst)` before and after access;
- use `msync(..., MS_SYNC)` after write;
- print device, offset, length, seed, SHA-256, and pass/fail JSON;
- reject any length other than 64 for this sentinel tool.

- [ ] **Step 3: Write the exact guest setup script**

The setup script must:

1. `modprobe cxl_type2_accel enable_cache=0 enable_memdev=1 use_dvsec_hdm=1 allow_uncommitted_hdm=0`.
2. Require exactly one PCI `8086:0d92`.
3. Require exactly one `mem*` owned by that endpoint.
4. Require exactly one RAM-capable root decoder with at least 256 MiB available.
5. Run:

```bash
cxl create-region -m -t ram -d "$DECODER" -w 1 -g 1024 -s 256M "$MEMDEV"
daxctl create-device -r "$REGION"
```

6. Require one `/dev/dax*`.
7. Read CFMWS/root-decoder HPA, region resource, DAX size/resource, memdev capacity, and endpoint DVSEC dump.
8. Emit one JSON file with:

```json
{"pci_bdf":"0000:36:00.0","vendor":"0x8086","device":"0x0d92","memdev":"mem0","root_decoder":"decoder0.0","region":"region0","dax":"dax0.0","capacity":268435456,"dax_size":268435456,"work_dpa_start":83886080,"work_dpa_end":117440512}
```

The values are discovered at runtime; the shown names are schema examples, not hardcoded selections.

- [ ] **Step 4: Add strict non-bypass arithmetic**

Parse QEMU's realized log values and require:

```text
bulk_end <= 83886080
117440512 <= coherent_pool_base
117440512 <= device_mem_size
work_dpa_end <= dax_size
cfmws_size == 268435456
```

Abort if `/sys/bus/pci/devices/$BDF/resource4` is opened by the helper process or if the selected device path is not `/dev/dax*`.

- [ ] **Step 5: Run host tests and shell syntax checks**

Run:

```bash
bash -n /tmp/slugarch-type2-transport-src/CXLMemSim/qemu_integration/slugarch_type2_guest_setup.sh
cc -O2 -Wall -Wextra -Werror -std=c11 -o /tmp/slugarch_type2_dax_sentinel /tmp/slugarch-type2-transport-src/CXLMemSim/qemu_integration/slugarch_type2_dax_sentinel.c -lcrypto
truncate -s 128M /tmp/slugarch_type2_dax_test.bin
/tmp/slugarch_type2_dax_sentinel --device /tmp/slugarch_type2_dax_test.bin --offset 83886080 --length 64 --mode write --seed 0x534c5432
/tmp/slugarch_type2_dax_sentinel --device /tmp/slugarch_type2_dax_test.bin --offset 83886080 --length 64 --mode read-verify --seed 0x534c5432
```

Expected: syntax and compiler checks pass; write and verify JSON both report pass.

- [ ] **Step 6: Commit guest tools**

Run:

```bash
git -C /tmp/slugarch-type2-transport-src/CXLMemSim add qemu_integration/slugarch_type2_guest_setup.sh qemu_integration/slugarch_type2_dax_sentinel.c
git -C /tmp/slugarch-type2-transport-src/CXLMemSim commit -m "test: add Type-2 devdax sentinel tools"
```

---

### Task 14: Prove the live two-way devdax/oracle sentinel

**Files:**

- Create: `/tmp/slugarch-type2-transport-src/CXLMemSim/qemu_integration/slugarch_type2_live_sentinel.sh`

- [ ] **Step 1: Write the immutable run-directory and preflight contract**

The script requires:

```text
QEMU=/tmp/slugarch-type2-transport-build/qemu/qemu-system-x86_64
SERVER=/tmp/slugarch-type2-transport-build/server/cxlmemsim_server
ORACLE=/tmp/slugarch-type2-transport-build/server/slugarch_type2_oracle
KERNEL=/tmp/slugarch-type2-transport-src/cxl/arch/x86/boot/bzImage
IMAGE=/tmp/slugarch-type2-transport-build/guest/slugarch-type2.img
RUN=/tmp/slugarch-type2-live-sentinel
SERVER_CPUS=4-5
QEMU_CPUS=0-3
RUN_ID=slugarch_type2_live_20260724
```

It fails if `RUN` exists, creates it once, records SHA-256 for all five inputs, copies complete commands into `RUN/commands`, and never overwrites a result file.

Create a private writable overlay and verify its backing path before launch:

```bash
qemu-img create -q -f qcow2 -F raw -b "$IMAGE" "$RUN/root.qcow2"
qemu-img info --output=json "$RUN/root.qcow2" > "$RUN/root.qcow2.info.json"
jq -e --arg image "$IMAGE" \
  '.format == "qcow2" and ."backing-filename" == $image' \
  "$RUN/root.qcow2.info.json"
```

The hashed patched raw image remains immutable; only the attempt-local overlay
is writable.

- [ ] **Step 2: Launch the exact server and TCG topology**

Server:

```bash
taskset -c "$SERVER_CPUS" "$SERVER" \
  --comm-mode=tcp \
  --port=10199 \
  --capacity=256 \
  --default_latency=400 \
  --slugarch-type2-protocol=true \
  --slugarch-event-log="$RUN/server-events.jsonl.gz" \
  --slugarch-shm-name="/slugarch_type2_live_${RUN_ID}"
```

QEMU:

```bash
taskset -c "$QEMU_CPUS" "$QEMU" \
  -accel tcg \
  -cpu max \
  -machine q35,cxl=on,cxl-fmw.0.targets.0=cxl.0,cxl-fmw.0.size=256M \
  -m 2G -smp 2 \
  -kernel "$KERNEL" \
  -append "root=/dev/vda rw console=ttyS0,115200 nokaslr systemd.mask=cxl-numa-setup.service" \
  -drive "file=$RUN/root.qcow2,if=none,id=bootdisk,format=qcow2" \
  -device virtio-blk-pci,drive=bootdisk,bus=pcie.0 \
  -qmp "unix:$RUN/qmp.sock,server=on,wait=off" \
  -netdev user,id=net0,hostfwd=tcp:127.0.0.1:12022-:22 \
  -device virtio-net-pci,netdev=net0,bus=pcie.0,mac=52:54:00:00:10:22 \
  -device pxb-cxl,id=cxl.0,bus=pcie.0,bus_nr=12 \
  -device cxl-rp,id=type2_rp,bus=cxl.0,chassis=0,slot=2 \
  -device cxl-type2,id=cxl-type2-slugarch,bus=type2_rp,sn=200,gpu-mode=0,coherency-enabled=false,cache-size=128M,mem-size=256M,sync-type2-wire=on,type2-wire-version=1,slugarch-event-log="$RUN/qemu-events.jsonl",cxlmemsim-addr=127.0.0.1,cxlmemsim-port=10199 \
  -nographic
```

Require TCG in the QEMU log; reject KVM and `icount`.

- [ ] **Step 3: Capture the expected pre-fix failure before accepting a pass**

On an unpatched kernel/QEMU pair, retain the boot log showing one of:

```text
memory-device mailbox registers are not mapped
CXL memdev/HDM/DAX registration skipped
```

Expected: no `/dev/dax*`; mark this diagnostic run failed. This is the precise failure the kernel task repairs and is not paper evidence.

- [ ] **Step 4: Deploy setup and sentinel binaries**

Build the guest helper and copy both files over SSH:

```bash
cc -O2 -Wall -Wextra -Werror -std=c11 -o "$RUN/slugarch_type2_dax_sentinel" /tmp/slugarch-type2-transport-src/CXLMemSim/qemu_integration/slugarch_type2_dax_sentinel.c -lcrypto
scp -P 12022 -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null /tmp/slugarch-type2-transport-src/CXLMemSim/qemu_integration/slugarch_type2_guest_setup.sh "$RUN/slugarch_type2_dax_sentinel" root@127.0.0.1:/root/
ssh -p 12022 -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null root@127.0.0.1 "bash /root/slugarch_type2_guest_setup.sh > /root/slugarch-type2-topology.json"
scp -P 12022 -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null root@127.0.0.1:/root/slugarch-type2-topology.json "$RUN/topology.json"
DAX="$(jq -er '.dax | select(type == "string" and startswith("dax"))' "$RUN/topology.json")"
```

Expected: topology JSON reports one `8086:0d92`, one memdev, one committed region, one 256 MiB devdax mapping, and the 80–112 MiB work window.

- [ ] **Step 5: Require zero QEMU memory counters before the exchange**

Connect to `$RUN/qmp.sock`, execute `qmp_capabilities`, and issue:

```json
{"execute":"qom-get","arguments":{"path":"/machine/peripheral/cxl-type2-slugarch","property":"slugarch-client-id"}}
{"execute":"qom-get","arguments":{"path":"/machine/peripheral/cxl-type2-slugarch","property":"slugarch-completed-reads"}}
{"execute":"qom-get","arguments":{"path":"/machine/peripheral/cxl-type2-slugarch","property":"slugarch-completed-writes"}}
{"execute":"qom-get","arguments":{"path":"/machine/peripheral/cxl-type2-slugarch","property":"slugarch-direct-cfmws"}}
```

The script's Python QMP helper saves every request and response in
`$RUN/qmp-before.jsonl`. Take `QEMU_CLIENT_ID` from the first response and run:

```bash
"$ORACLE" --host 127.0.0.1 --port 10199 --role oracle --snapshot-client "$QEMU_CLIENT_ID" --json "$RUN/counters-before.json"
```

Expected: completed reads/writes/bytes/failures/in-flight are all zero.

- [ ] **Step 6: Perform oracle-to-guest direction**

Run:

```bash
"$ORACLE" --host 127.0.0.1 --port 10199 --role oracle --write-pattern 83886080 64 0x534c5432 --json "$RUN/oracle-write.json"
qmp_set "/machine/peripheral/cxl-type2-slugarch" "slugarch-phase-id" "sentinel_oracle_to_guest"
ssh -p 12022 -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null root@127.0.0.1 "/root/slugarch_type2_dax_sentinel --device /dev/$DAX --offset 83886080 --length 64 --mode read-verify --seed 0x534c5432" > "$RUN/guest-read.json"
qmp_set "/machine/peripheral/cxl-type2-slugarch" "slugarch-phase-id" "idle"
```

Expected: guest reports pass and the same SHA-256 as the oracle write.
Because the CFMWS `MemoryRegionOps` maximum access size is 8 bytes, the
64-byte line produces exactly eight QEMU-client READ transactions and 64 read
bytes; oracle counters are separate.

- [ ] **Step 7: Perform guest-to-oracle direction**

Run:

```bash
qmp_set "/machine/peripheral/cxl-type2-slugarch" "slugarch-phase-id" "sentinel_guest_to_oracle"
ssh -p 12022 -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null root@127.0.0.1 "/root/slugarch_type2_dax_sentinel --device /dev/$DAX --offset 83886080 --length 64 --mode write --seed 0xa5a5a5a5" > "$RUN/guest-write.json"
"$ORACLE" --host 127.0.0.1 --port 10199 --role oracle --read-verify-pattern 83886080 64 0xa5a5a5a5 --json "$RUN/oracle-read.json"
qmp_set "/machine/peripheral/cxl-type2-slugarch" "slugarch-phase-id" "idle"
```

Expected: oracle reports pass and the same SHA-256 as the guest write. The
64-byte line produces exactly eight QEMU-client WRITE transactions and 64
written bytes.

- [ ] **Step 8: Join evidence and enforce path counters**

Decompress server JSONL and join with QEMU delay lines on `(client_id, request_id, server_sequence)`.

Require:

```text
QEMU completed reads = 8
QEMU completed writes = 8
QEMU read bytes = 64
QEMU written bytes = 64
QEMU failed requests = 0
QEMU in flight = 0
QEMU direct CFMWS completions = 16
QEMU delay events = 16
QEMU delay undershoots = 0
QEMU BAR4 overlay completions = 0
QEMU bulk overlay completions = 0
QEMU coherent pool completions = 0
QEMU local shadow completions = 0
QEMU local cache completions = 0
missing joins = 0
duplicate joins = 0
payload digest mismatches = 0
```

Also require no guest process opens `resource4` and no QEMU log claims a CFMWS access used BAR4.

- [ ] **Step 9: Run the live smoke**

Run:

```bash
QEMU=/tmp/slugarch-type2-transport-build/qemu/qemu-system-x86_64 SERVER=/tmp/slugarch-type2-transport-build/server/cxlmemsim_server ORACLE=/tmp/slugarch-type2-transport-build/server/slugarch_type2_oracle KERNEL=/tmp/slugarch-type2-transport-src/cxl/arch/x86/boot/bzImage IMAGE=/tmp/slugarch-type2-transport-build/guest/slugarch-type2.img RUN=/tmp/slugarch-type2-live-sentinel bash /tmp/slugarch-type2-transport-src/CXLMemSim/qemu_integration/slugarch_type2_live_sentinel.sh
```

Expected: exit `0` only after both sentinel directions and every counter/join assertion pass. Otherwise retain the run directory with `FAILED.json`.

- [ ] **Step 10: Commit the live proof harness**

Run:

```bash
git -C /tmp/slugarch-type2-transport-src/CXLMemSim add qemu_integration/slugarch_type2_live_sentinel.sh
git -C /tmp/slugarch-type2-transport-src/CXLMemSim commit -m "test: prove live Type-2 CXL.mem sentinel"
```

---

### Task 15: Run the complete transport gate and freeze hashes

**Files:**

- Create artifact: `/tmp/slugarch-type2-transport-proof/`
- No source changes unless a failing gate identifies a bug and the affected task is repeated with a new scoped commit.

- [ ] **Step 1: Run all server tests from a clean build**

Run:

```bash
test ! -e /tmp/slugarch-type2-transport-proof
mkdir -p /tmp/slugarch-type2-transport-proof/server-build
cmake -S /tmp/slugarch-type2-transport-src/CXLMemSim -B /tmp/slugarch-type2-transport-proof/server-build -DCMAKE_BUILD_TYPE=Release
cmake --build /tmp/slugarch-type2-transport-proof/server-build -j
ctest --test-dir /tmp/slugarch-type2-transport-proof/server-build --output-on-failure
```

Expected: configure, build, and every registered test pass.

- [ ] **Step 2: Run all relevant QEMU tests from a clean build**

Run:

```bash
mkdir -p /tmp/slugarch-type2-transport-proof/qemu-build
cd /tmp/slugarch-type2-transport-proof/qemu-build
/tmp/slugarch-type2-transport-src/CXLMemSim/lib/qemu/configure --target-list=x86_64-softmmu
ninja qemu-system-x86_64 tests/unit/test-cxl-type2-wire tests/unit/test-cxl-type2-route tests/qtest/cxl-test
./tests/unit/test-cxl-type2-wire --tap -k
./tests/unit/test-cxl-type2-route --tap -k
env QTEST_QEMU_BINARY="$PWD/qemu-system-x86_64" ./tests/qtest/cxl-test --tap -k
```

Expected: all new unit tests and the full existing CXL qtest binary pass.

- [ ] **Step 3: Run protocol, no-guest, and live sentinel gates in order**

Run:

```bash
SERVER=/tmp/slugarch-type2-transport-proof/server-build/cxlmemsim_server ORACLE=/tmp/slugarch-type2-transport-proof/server-build/slugarch_type2_oracle bash /tmp/slugarch-type2-transport-src/CXLMemSim/qemu_integration/slugarch_type2_protocol_smoke.sh
QEMU=/tmp/slugarch-type2-transport-proof/qemu-build/qemu-system-x86_64 SERVER=/tmp/slugarch-type2-transport-proof/server-build/cxlmemsim_server ORACLE=/tmp/slugarch-type2-transport-proof/server-build/slugarch_type2_oracle bash /tmp/slugarch-type2-transport-src/CXLMemSim/qemu_integration/slugarch_type2_no_guest_smoke.sh
QEMU=/tmp/slugarch-type2-transport-proof/qemu-build/qemu-system-x86_64 SERVER=/tmp/slugarch-type2-transport-proof/server-build/cxlmemsim_server ORACLE=/tmp/slugarch-type2-transport-proof/server-build/slugarch_type2_oracle KERNEL=/tmp/slugarch-type2-transport-src/cxl/arch/x86/boot/bzImage IMAGE=/tmp/slugarch-type2-transport-build/guest/slugarch-type2.img RUN=/tmp/slugarch-type2-transport-proof/live-sentinel bash /tmp/slugarch-type2-transport-src/CXLMemSim/qemu_integration/slugarch_type2_live_sentinel.sh
```

Expected: all three scripts exit `0`. Do not proceed to paper measurements on a partial pass.

- [ ] **Step 4: Verify source cleanliness and scoped history**

Run:

```bash
git -C /tmp/slugarch-type2-transport-src/CXLMemSim status --short
git -C /tmp/slugarch-type2-transport-src/CXLMemSim/lib/qemu status --short
git -C /tmp/slugarch-type2-transport-src/cxl status --short
git -C /tmp/slugarch-type2-transport-src/CXLMemSim log --oneline --decorate -8
git -C /tmp/slugarch-type2-transport-src/CXLMemSim/lib/qemu log --oneline --decorate -8
git -C /tmp/slugarch-type2-transport-src/cxl log --oneline --decorate -4
```

Expected:

- CXLMemSim and QEMU contain only intentional commits.
- The kernel still has the unrelated captured baseline changes unstaged; their patch hash remains recorded.
- No generated binaries, logs, images, or result directories are staged.

- [ ] **Step 5: Freeze the transport proof inputs**

Run:

```bash
mkdir -p /tmp/slugarch-type2-transport-proof/frozen
git -C /tmp/slugarch-type2-transport-src/CXLMemSim rev-parse HEAD > /tmp/slugarch-type2-transport-proof/frozen/cxlmemsim.head
git -C /tmp/slugarch-type2-transport-src/CXLMemSim/lib/qemu rev-parse HEAD > /tmp/slugarch-type2-transport-proof/frozen/qemu.head
git -C /tmp/slugarch-type2-transport-src/cxl rev-parse HEAD > /tmp/slugarch-type2-transport-proof/frozen/kernel.head
sha256sum /tmp/slugarch-type2-transport-proof/server-build/cxlmemsim_server /tmp/slugarch-type2-transport-proof/server-build/slugarch_type2_oracle /tmp/slugarch-type2-transport-proof/qemu-build/qemu-system-x86_64 /tmp/slugarch-type2-transport-src/cxl/arch/x86/boot/bzImage /tmp/slugarch-type2-transport-build/guest/slugarch-type2.img > /tmp/slugarch-type2-transport-proof/frozen/binaries.sha256
cp -a /tmp/slugarch-type2-transport-src/source-capture /tmp/slugarch-type2-transport-proof/frozen/
find /tmp/slugarch-type2-transport-proof/frozen -type f -print0 | sort -z | xargs -0 sha256sum > /tmp/slugarch-type2-transport-proof/FROZEN_SHA256SUMS
sha256sum -c /tmp/slugarch-type2-transport-proof/FROZEN_SHA256SUMS
```

Expected: every frozen file verifies. These hashes become inputs to the later pilot/campaign manifest; this transport proof alone does not authorize paper numbers.

- [ ] **Step 6: Publish the verified campaign inputs**

Fail if the handoff root already exists, then publish only the just-verified
files:

```bash
test ! -e /tmp/slugarch-type2-cxlmem-build
mkdir -p /tmp/slugarch-type2-cxlmem-build/qemu
install -m 0555 /tmp/slugarch-type2-transport-proof/qemu-build/qemu-system-x86_64 /tmp/slugarch-type2-cxlmem-build/qemu/qemu-system-x86_64
install -m 0555 /tmp/slugarch-type2-transport-proof/server-build/cxlmemsim_server /tmp/slugarch-type2-cxlmem-build/cxlmemsim_server
install -m 0555 /tmp/slugarch-type2-transport-proof/server-build/slugarch_type2_oracle /tmp/slugarch-type2-cxlmem-build/slugarch_type2_oracle
install -m 0444 /tmp/slugarch-type2-transport-src/cxl/arch/x86/boot/bzImage /tmp/slugarch-type2-cxlmem-build/bzImage
cp --reflink=auto --sparse=always /tmp/slugarch-type2-transport-build/guest/slugarch-type2.img /tmp/slugarch-type2-cxlmem-build/slugarch-type2.img
chmod 0444 /tmp/slugarch-type2-cxlmem-build/slugarch-type2.img
sha256sum /tmp/slugarch-type2-cxlmem-build/qemu/qemu-system-x86_64 /tmp/slugarch-type2-cxlmem-build/cxlmemsim_server /tmp/slugarch-type2-cxlmem-build/slugarch_type2_oracle /tmp/slugarch-type2-cxlmem-build/bzImage /tmp/slugarch-type2-cxlmem-build/slugarch-type2.img > /tmp/slugarch-type2-cxlmem-build/TRANSPORT_SHA256SUMS
(cd / && sha256sum -c /tmp/slugarch-type2-cxlmem-build/TRANSPORT_SHA256SUMS)
```

Expected: all five published inputs verify. The campaign consumes these exact
paths; it must not silently substitute a binary or the older module-bearing
base image.

---

## Self-review checklist

- Protocol coverage: all eight frame types, fixed sizes, little-endian fields, CRC-32C, monotonic IDs, roles, exact I/O, and five-second deadlines have tests.
- Authority coverage: server bytes supply reads; writes complete on the server; failures return `MEMTX_ERROR`; no reconnect hides a failed boot.
- Counter coverage: QEMU and oracle clients are separate, snapshots are nondestructive, in-flight is observable, and QEMU/server counts are joined.
- Evidence coverage: gzip server completion/error events and QEMU delay events carry the complete join key plus payload SHA-256.
- Delay coverage: QEMU uses `CLOCK_MONOTONIC_RAW`, applies the full returned latency without subtracting RPC time, rejects delays above 1 ms, and records undershoot/overshoot.
- Topology coverage: exactly one CFMWS, target, direct root port, Type-2 endpoint, one-way interleave, zero DPA base, and 256 MiB capacity are enforced.
- Bypass coverage: the measured line is at 80 MiB, no `resource4` mapping is used, and direct/overlay/shadow/cache counters prove the path.
- DVSEC coverage: cache capacity is only `cap2`; exactly one active 256 MiB memory range exists.
- Kernel coverage: QEMU uses two repeatable active-DVSEC snapshots, zero-based DPA is separated from region-assigned HPA, and the complete matching module set is installed into a copied image.
- Regression coverage: legacy server and Type-2 defaults remain off and existing Type-3 CFMWS qtests remain green.
- Isolation coverage: dirty originals are captured and never edited or built; generated artifacts remain outside source trees.
- Claim boundary: a passing plan proves one QEMU/CXLMemSim simulator path and no hardware, CXL.cache, device-side SlugArch execution, or physical-latency claim.
