module hj_policy_tb;
  reg clk = 1'b0;
  reg rst_n = 1'b0;

  reg h2d_flit_valid = 1'b0;
  wire h2d_flit_ready;
  reg [511:0] h2d_flit_data = 512'd0;
  wire ep_flit_valid;
  reg ep_flit_ready = 1'b1;
  wire [511:0] ep_flit_data;

  reg ep_resp_valid = 1'b0;
  wire ep_resp_ready;
  reg [511:0] ep_resp_data = 512'd0;
  wire d2h_flit_valid;
  reg d2h_flit_ready = 1'b1;
  wire [511:0] d2h_flit_data;

  reg policy_load_begin = 1'b0;
  reg policy_load_valid = 1'b0;
  wire policy_load_ready;
  reg [15:0] policy_load_index = 16'd0;
  reg [127:0] policy_load_word = 128'd0;
  reg policy_load_commit = 1'b0;
  reg policy_load_abort = 1'b0;
  reg [255:0] policy_load_digest = 256'd0;
  reg [31:0] policy_load_instruction_count = 32'd0;
  reg [31:0] policy_load_range_count = 32'd0;

  wire policy_ready;
  wire [31:0] policy_error;
  wire [255:0] policy_digest;
  wire record_valid;
  reg record_ready = 1'b0;
  wire [15:0] record_length;
  wire [1023:0] record_data;
  wire [63:0] event_count;
  wire [63:0] record_count;
  wire [63:0] metadata_bytes;
  wire [63:0] reject_count;
  wire [63:0] instruction_count;
  wire [63:0] epoch;
  wire [63:0] app_flit_bytes;
  wire [63:0] stall_cycles;
  wire [63:0] metadata_drop_count;
  wire [15:0] last_reject_code;

  reg [127:0] base_image [0:39];
  reg [127:0] work_image [0:39];
  reg [1023:0] held_record;
  reg [15:0] held_length;
  string policy_hex;
  integer i;
  integer wait_cycles;

  slugcxl_hj_pipeline u_dut (
    .clk(clk),
    .rst_n(rst_n),
    .h2d_flit_valid(h2d_flit_valid),
    .h2d_flit_ready(h2d_flit_ready),
    .h2d_flit_data(h2d_flit_data),
    .ep_flit_valid(ep_flit_valid),
    .ep_flit_ready(ep_flit_ready),
    .ep_flit_data(ep_flit_data),
    .ep_resp_valid(ep_resp_valid),
    .ep_resp_ready(ep_resp_ready),
    .ep_resp_data(ep_resp_data),
    .d2h_flit_valid(d2h_flit_valid),
    .d2h_flit_ready(d2h_flit_ready),
    .d2h_flit_data(d2h_flit_data),
    .policy_load_begin(policy_load_begin),
    .policy_load_valid(policy_load_valid),
    .policy_load_ready(policy_load_ready),
    .policy_load_index(policy_load_index),
    .policy_load_word(policy_load_word),
    .policy_load_commit(policy_load_commit),
    .policy_load_abort(policy_load_abort),
    .policy_load_digest(policy_load_digest),
    .policy_load_instruction_count(policy_load_instruction_count),
    .policy_load_range_count(policy_load_range_count),
    .policy_ready(policy_ready),
    .policy_error(policy_error),
    .policy_digest(policy_digest),
    .record_valid(record_valid),
    .record_ready(record_ready),
    .record_length(record_length),
    .record_data(record_data),
    .event_count(event_count),
    .record_count(record_count),
    .metadata_bytes(metadata_bytes),
    .reject_count(reject_count),
    .instruction_count(instruction_count),
    .epoch(epoch),
    .app_flit_bytes(app_flit_bytes),
    .stall_cycles(stall_cycles),
    .metadata_drop_count(metadata_drop_count),
    .last_reject_code(last_reject_code)
  );

  task automatic cycle;
    begin
      #1 clk = 1'b1;
      #1 clk = 1'b0;
    end
  endtask

  task automatic reset_dut;
    begin
      rst_n = 1'b0;
      h2d_flit_valid = 1'b0;
      h2d_flit_data = 512'd0;
      ep_resp_valid = 1'b0;
      record_ready = 1'b0;
      policy_load_begin = 1'b0;
      policy_load_valid = 1'b0;
      policy_load_commit = 1'b0;
      policy_load_abort = 1'b0;
      repeat (4) cycle();
      rst_n = 1'b1;
      cycle();
      if (policy_ready || policy_error != 32'd0)
        $fatal(1, "reset did not clear policy state");
    end
  endtask

  task automatic restore_image;
    begin
      for (i = 0; i < 40; i = i + 1)
        work_image[i] = base_image[i];
    end
  endtask

  task automatic begin_load;
    begin
      policy_load_digest = {work_image[3], work_image[2]};
      policy_load_instruction_count = work_image[1][31:0];
      policy_load_range_count = work_image[1][63:32];
      policy_load_begin = 1'b1;
      cycle();
      policy_load_begin = 1'b0;
    end
  endtask

  task automatic send_load_word(input integer index);
    begin
      policy_load_index = index[15:0];
      policy_load_word = work_image[index];
      policy_load_valid = 1'b1;
      #1;
      if (!policy_load_ready)
        $fatal(1, "loader not ready for word %0d", index);
      clk = 1'b1;
      #1 clk = 1'b0;
      policy_load_valid = 1'b0;
    end
  endtask

  task automatic commit_load;
    begin
      policy_load_commit = 1'b1;
      cycle();
      policy_load_commit = 1'b0;
      cycle();
    end
  endtask

  task automatic load_work_image;
    begin
      begin_load();
      for (i = 0; i < 40; i = i + 1)
        send_load_word(i);
      commit_load();
    end
  endtask

  task automatic make_event;
    begin
      h2d_flit_data = 512'd0;
      h2d_flit_data[7:0] = 8'h20;
      h2d_flit_data[23:8] = 16'd7;
      h2d_flit_data[87:24] = 64'd83886080;
      h2d_flit_data[95:88] = 8'd1;
      h2d_flit_data[103:96] = 8'd2;
      h2d_flit_data[111:104] = 8'd3;
      h2d_flit_data[119:112] = 8'd4;
      h2d_flit_data[127:120] = 8'd5;
      h2d_flit_data[135:128] = 8'd6;
      h2d_flit_data[143:136] = 8'd7;
      h2d_flit_data[151:144] = 8'd8;
      h2d_flit_data[407:344] = 64'd42;
      h2d_flit_data[471:408] = 64'd3;
      h2d_flit_data[503:472] = 32'd0;
      h2d_flit_data[511:504] = 8'd8;
    end
  endtask

  task automatic send_event;
    begin
      make_event();
      drive_event();
    end
  endtask

  task automatic drive_event;
    begin
      h2d_flit_valid = 1'b1;
      #1;
      if (!h2d_flit_ready)
        $fatal(1, "interpreter did not accept event");
      clk = 1'b1;
      #1 clk = 1'b0;
      h2d_flit_valid = 1'b0;
    end
  endtask

  task automatic wait_for_record;
    begin
      wait_cycles = 0;
      while (!record_valid
             && (policy_error == 32'd0)
             && (wait_cycles < 200)) begin
        cycle();
        wait_cycles = wait_cycles + 1;
      end
      if (!record_valid)
        $fatal(1, "expected record, policy_error=%0d", policy_error);
    end
  endtask

  task automatic consume_record;
    begin
      record_ready = 1'b1;
      cycle();
      record_ready = 1'b0;
      cycle();
    end
  endtask

  task automatic expect_current_event_accepts_without_record;
    begin
      drive_event();
      wait_cycles = 0;
      while ((event_count == 64'd0)
             && (policy_error == 32'd0)
             && (wait_cycles < 200)) begin
        cycle();
        wait_cycles = wait_cycles + 1;
      end
      if (event_count != 64'd1
          || policy_error != 32'd0
          || record_valid
          || record_count != 64'd0)
        $fatal(1, "non-matching branch did not accept without record");
    end
  endtask

  task automatic insert_match_instruction(
    input [7:0] opcode,
    input [7:0] arg0,
    input [31:0] arg1
  );
    begin
      work_image[8] = work_image[7];
      work_image[7] = work_image[6];
      work_image[6] = work_image[5];
      work_image[5] = work_image[4];
      work_image[4] = 128'd0;
      work_image[4][7:0] = opcode;
      work_image[4][15:8] = arg0;
      work_image[4][23:16] = 8'd3;
      work_image[4][63:32] = arg1;
      work_image[1][31:0] = 32'd5;
    end
  endtask

  task automatic expect_runtime_error(input [31:0] expected);
    begin
      send_event();
      wait_cycles = 0;
      while ((policy_error == 32'd0) && (wait_cycles < 200)) begin
        cycle();
        wait_cycles = wait_cycles + 1;
      end
      if (policy_error != expected)
        $fatal(1, "runtime error %0d, expected %0d",
               policy_error, expected);
      if (record_valid)
        $fatal(1, "failing policy emitted a success record");
      if (metadata_drop_count != 64'd1)
        $fatal(1, "failing event was not accounted as one drop");
    end
  endtask

  initial begin
    if (!$value$plusargs("POLICY_HEX=%s", policy_hex))
      $fatal(1, "missing +POLICY_HEX=<path>");
    $readmemh(policy_hex, base_image);

    // Normal four-op policy: capture, emit, epoch-from-phase, halt.
    restore_image();
    reset_dut();
    load_work_image();
    if (!policy_ready || policy_error != 32'd0)
      $fatal(1, "valid image did not commit");
    if (policy_digest != {work_image[3], work_image[2]})
      $fatal(1, "active digest differs from loaded header");
    send_event();
    wait_for_record();
    if (record_length != 16'd104)
      $fatal(1, "record length %0d, expected 104", record_length);
    if (record_data[159:96] != 64'd42)
      $fatal(1, "record event id mismatch");
    if (record_data[479:416] != 64'd3)
      $fatal(1, "record epoch mismatch");
    if (record_data[679:672] != 8'd0
        || record_data[687:680] != 8'd8
        || record_data[695:688] != 8'd8)
      $fatal(1, "validation capture header mismatch");
    if (record_data[831:768] != 64'h7eb5108b368a78ed)
      $fatal(1, "validation FNV-1a hash mismatch");
    held_record = record_data;
    held_length = record_length;
    repeat (4) begin
      cycle();
      if (!record_valid
          || record_data != held_record
          || record_length != held_length)
        $fatal(1, "record changed while backpressured");
    end
    if (stall_cycles < 64'd4)
      $fatal(1, "backpressure stalls were not counted");
    consume_record();
    if (record_count != 64'd1
        || event_count != 64'd1
        || metadata_bytes != 64'd8
        || instruction_count != 64'd4
        || epoch != 64'd3
        || metadata_drop_count != 64'd0)
      $fatal(1, "normal policy counters mismatch");

    // Every match opcode takes its matching path to one record.
    restore_image();
    insert_match_instruction(8'h01, 8'd2, 32'd0);
    reset_dut();
    load_work_image();
    send_event();
    wait_for_record();
    consume_record();
    if (record_count != 64'd1 || instruction_count != 64'd5)
      $fatal(1, "MATCH_CLASS matching path mismatch");

    restore_image();
    insert_match_instruction(8'h02, 8'd0, 32'd0);
    reset_dut();
    load_work_image();
    send_event();
    wait_for_record();
    consume_record();
    if (record_count != 64'd1)
      $fatal(1, "MATCH_DIRECTION matching path mismatch");

    restore_image();
    insert_match_instruction(8'h03, 8'd0, 32'd0);
    reset_dut();
    load_work_image();
    send_event();
    wait_for_record();
    consume_record();
    if (record_count != 64'd1)
      $fatal(1, "MATCH_STATUS matching path mismatch");

    restore_image();
    insert_match_instruction(8'h04, 8'd0, 32'd0);
    reset_dut();
    load_work_image();
    send_event();
    wait_for_record();
    consume_record();
    if (record_count != 64'd1)
      $fatal(1, "MATCH_RANGE matching path mismatch");

    restore_image();
    insert_match_instruction(8'h05, 8'd0, 32'd2);
    reset_dut();
    load_work_image();
    send_event();
    wait_for_record();
    consume_record();
    if (record_count != 64'd1)
      $fatal(1, "SAMPLE matching path mismatch");

    // A false SAMPLE branch skips capture/emit/epoch and terminates at HALT.
    reset_dut();
    load_work_image();
    make_event();
    h2d_flit_data[407:344] = 64'd41;
    expect_current_event_accepts_without_record();
    if (instruction_count != 64'd2)
      $fatal(1, "SAMPLE false-path instruction count mismatch");

    // DELTA records contain exact little-endian (index,value) pairs.
    restore_image();
    work_image[4][15:8] = 8'd1;
    reset_dut();
    load_work_image();
    send_event();
    wait_for_record();
    if (record_length != 16'd112
        || record_data[679:672] != 8'd1
        || record_data[695:688] != 8'd16
        || record_data[703:696] != 8'd8
        || record_data[783:768] != 16'h0100
        || record_data[799:784] != 16'h0201)
      $fatal(1, "CAPTURE DELTA encoding mismatch");
    consume_record();
    if (metadata_bytes != 64'd16)
      $fatal(1, "CAPTURE DELTA metadata count mismatch");

    // FULL copies an exact 32-byte declared prefix into the fixed record.
    restore_image();
    work_image[4][15:8] = 8'd2;
    reset_dut();
    load_work_image();
    make_event();
    for (i = 0; i < 32; i = i + 1)
      h2d_flit_data[88 + i * 8 +: 8] = i[7:0] + 8'd1;
    h2d_flit_data[509:504] = 6'd32;
    drive_event();
    wait_for_record();
    if (record_length != 16'd128
        || record_data[679:672] != 8'd2
        || record_data[695:688] != 8'd32
        || record_data[775:768] != 8'd1
        || record_data[1023:1016] != 8'd32)
      $fatal(1, "CAPTURE FULL encoding mismatch");
    consume_record();
    if (metadata_bytes != 64'd32)
      $fatal(1, "CAPTURE FULL metadata count mismatch");

    // EPOCH_INCREMENT is persistent across observations.
    restore_image();
    work_image[6] = 128'd0;
    work_image[6][7:0] = 8'h08;
    reset_dut();
    load_work_image();
    send_event();
    wait_for_record();
    if (record_data[479:416] != 64'd1)
      $fatal(1, "first EPOCH_INCREMENT mismatch");
    consume_record();
    send_event();
    wait_for_record();
    if (record_data[479:416] != 64'd2)
      $fatal(1, "second EPOCH_INCREMENT mismatch");
    consume_record();

    // REJECT is a non-error terminal decision with an exact user code.
    restore_image();
    work_image[5] = 128'd0;
    work_image[5][7:0] = 8'h0a;
    work_image[5][63:32] = 32'd7;
    reset_dut();
    load_work_image();
    send_event();
    wait_cycles = 0;
    while ((reject_count == 64'd0)
           && (policy_error == 32'd0)
           && (wait_cycles < 200)) begin
      cycle();
      wait_cycles = wait_cycles + 1;
    end
    if (reject_count != 64'd1
        || event_count != 64'd1
        || last_reject_code != 16'd7
        || record_count != 64'd0
        || record_valid
        || policy_error != 32'd0)
      $fatal(1, "REJECT terminal decision mismatch");

    // Abort preserves the old bank and permits a clean subsequent load.
    restore_image();
    reset_dut();
    begin_load();
    send_load_word(0);
    policy_load_abort = 1'b1;
    cycle();
    policy_load_abort = 1'b0;
    load_work_image();
    if (!policy_ready || policy_error != 32'd0)
      $fatal(1, "aborted load prevented a subsequent valid commit");

    // Partial image commits fail with the exact structure-size code.
    restore_image();
    reset_dut();
    begin_load();
    send_load_word(0);
    commit_load();
    if (policy_error != 32'd2 || policy_ready)
      $fatal(1, "partial image error mismatch");

    // A header declaring 33 instructions fails before activation.
    restore_image();
    work_image[1][31:0] = 32'd33;
    reset_dut();
    load_work_image();
    if (policy_error != 32'd6 || policy_ready)
      $fatal(1, "33-instruction image error mismatch");

    // An out-of-range loader word is rejected with control-flow error 11.
    restore_image();
    reset_dut();
    begin_load();
    policy_load_index = 16'd40;
    policy_load_word = 128'd0;
    policy_load_valid = 1'b1;
    cycle();
    policy_load_valid = 1'b0;
    if (policy_error != 32'd11)
      $fatal(1, "out-of-range loader index error mismatch");
    commit_load();
    if (policy_error != 32'd11)
      $fatal(1, "loader error was not sticky through commit");

    // Unsupported opcode is fail-stop and cannot emit a record.
    restore_image();
    work_image[4] = 128'd0;
    work_image[4][7:0] = 8'hff;
    reset_dut();
    load_work_image();
    expect_runtime_error(32'd12);
    restore_image();
    load_work_image();
    if (!policy_ready || policy_error != 32'd0)
      $fatal(1, "valid reload did not recover a fail-stop engine");

    // Zero skip is malformed even when the comparison would match.
    restore_image();
    work_image[4] = 128'd0;
    work_image[4][7:0] = 8'h01;
    work_image[4][15:8] = 8'd2;
    reset_dut();
    load_work_image();
    expect_runtime_error(32'd11);

    // Range index one is invalid when the image exposes only range zero.
    restore_image();
    work_image[4] = 128'd0;
    work_image[4][7:0] = 8'h04;
    work_image[4][15:8] = 8'd1;
    work_image[4][23:16] = 8'd1;
    reset_dut();
    load_work_image();
    expect_runtime_error(32'd11);

    // Thirty-two non-terminal instructions hit the exact step watchdog.
    restore_image();
    work_image[1][31:0] = 32'd32;
    for (i = 4; i < 36; i = i + 1) begin
      work_image[i] = 128'd0;
      work_image[i][7:0] = 8'h09;
    end
    reset_dut();
    load_work_image();
    expect_runtime_error(32'd16);

    $display("SLUGARCH_HJ_POLICY_RTL_PASS");
    $finish;
  end
endmodule
