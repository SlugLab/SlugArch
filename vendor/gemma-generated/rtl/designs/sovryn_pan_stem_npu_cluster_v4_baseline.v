(* keep_hierarchy = "yes" *)
module sovryn_pan_stem_npu_dispatch_frontend(
  input wire clk,
  input wire rst_n,
  input wire dispatch_valid,
  input wire [2:0] dispatch_opcode,
  input wire dispatch_dest_tile,
  input wire [3:0] dispatch_addr,
  input wire [15:0] dispatch_data,
  output reg tile0_cmd_valid,
  output reg tile1_cmd_valid,
  output reg [2:0] tile0_cmd_opcode,
  output reg [2:0] tile1_cmd_opcode,
  output reg [3:0] tile0_cmd_addr,
  output reg [3:0] tile1_cmd_addr,
  output reg [15:0] tile0_cmd_data,
  output reg [15:0] tile1_cmd_data
);
  always @(posedge clk) begin
    if (!rst_n) begin
      tile0_cmd_valid <= 1'b0;
      tile1_cmd_valid <= 1'b0;
      tile0_cmd_opcode <= 3'd0;
      tile1_cmd_opcode <= 3'd0;
      tile0_cmd_addr <= 4'd0;
      tile1_cmd_addr <= 4'd0;
      tile0_cmd_data <= 16'd0;
      tile1_cmd_data <= 16'd0;
    end else begin
      tile0_cmd_valid <= dispatch_valid && !dispatch_dest_tile;
      tile1_cmd_valid <= dispatch_valid && dispatch_dest_tile;

      if (dispatch_valid && !dispatch_dest_tile) begin
        tile0_cmd_opcode <= dispatch_opcode;
        tile0_cmd_addr <= dispatch_addr;
        tile0_cmd_data <= dispatch_data;
      end

      if (dispatch_valid && dispatch_dest_tile) begin
        tile1_cmd_opcode <= dispatch_opcode;
        tile1_cmd_addr <= dispatch_addr;
        tile1_cmd_data <= dispatch_data;
      end
    end
  end
endmodule

(* keep_hierarchy = "yes" *)
module sovryn_pan_stem_npu_tile_scheduler(
  input wire clk,
  input wire rst_n,
  input wire cmd_valid,
  input wire [2:0] cmd_opcode,
  input wire [3:0] cmd_addr,
  input wire [15:0] cmd_data,
  output reg issue_valid,
  output reg [2:0] issue_opcode,
  output reg [3:0] issue_addr,
  output reg [15:0] issue_data
);
  always @(posedge clk) begin
    if (!rst_n) begin
      issue_valid <= 1'b0;
      issue_opcode <= 3'd0;
      issue_addr <= 4'd0;
      issue_data <= 16'd0;
    end else begin
      issue_valid <= cmd_valid;
      if (cmd_valid) begin
        issue_opcode <= cmd_opcode;
        issue_addr <= cmd_addr;
        issue_data <= cmd_data;
      end
    end
  end
endmodule

(* keep_hierarchy = "yes" *)
module sovryn_pan_stem_npu_mul_macro_v4(
  input wire clk,
  input wire rst_n,
  input wire start_valid,
  input wire [3:0] start_addr,
  input wire [15:0] start_multiplicand,
  input wire [15:0] start_repeat_count,
  output reg busy,
  output reg done_valid,
  output reg [3:0] done_addr,
  output reg [31:0] done_result
);
  reg [1:0] phase;
  reg [3:0] latched_addr;
  reg [4:0] steps_remaining;
  reg [15:0] multiplier_bits;
  reg [15:0] accumulator_low;
  reg [15:0] accumulator_high;
  reg [15:0] shifted_multiplicand_low;
  reg [15:0] shifted_multiplicand_high;
  reg carry;

  wire [16:0] low_sum;
  wire [16:0] high_sum;

  function [4:0] effective_steps;
    input [15:0] value;
    integer bit_index;
    begin
      effective_steps = 5'd0;
      for (bit_index = 0; bit_index < 16; bit_index = bit_index + 1) begin
        if (value[bit_index]) begin
          effective_steps = bit_index[4:0] + 5'd1;
        end
      end
    end
  endfunction

  assign low_sum = {1'b0, accumulator_low} + {1'b0, shifted_multiplicand_low};
  assign high_sum = {1'b0, accumulator_high}
    + {1'b0, shifted_multiplicand_high}
    + {16'd0, carry};

  always @(posedge clk) begin
    done_valid <= 1'b0;
    if (!rst_n) begin
      busy <= 1'b0;
      phase <= 2'd0;
      latched_addr <= 4'd0;
      steps_remaining <= 5'd0;
      multiplier_bits <= 16'd0;
      accumulator_low <= 16'd0;
      accumulator_high <= 16'd0;
      shifted_multiplicand_low <= 16'd0;
      shifted_multiplicand_high <= 16'd0;
      carry <= 1'b0;
      done_addr <= 4'd0;
      done_result <= 32'd0;
    end else if (busy) begin
      if (steps_remaining == 5'd0 && phase == 2'd0) begin
        busy <= 1'b0;
        done_valid <= 1'b1;
        done_addr <= latched_addr;
        done_result <= {accumulator_high, accumulator_low};
      end else begin
        case (phase)
          2'd0: begin
            if (multiplier_bits[0]) begin
              accumulator_low <= low_sum[15:0];
              carry <= low_sum[16];
              phase <= 2'd1;
            end else begin
              carry <= 1'b0;
              phase <= 2'd2;
            end
          end
          2'd1: begin
            accumulator_high <= high_sum[15:0];
            phase <= 2'd2;
          end
          default: begin
            shifted_multiplicand_low <= {shifted_multiplicand_low[14:0], 1'b0};
            shifted_multiplicand_high <= {
              shifted_multiplicand_high[14:0],
              shifted_multiplicand_low[15]
            };
            multiplier_bits <= {1'b0, multiplier_bits[15:1]};
            if (steps_remaining != 5'd0) begin
              steps_remaining <= steps_remaining - 5'd1;
            end
            carry <= 1'b0;
            phase <= 2'd0;
          end
        endcase
      end
    end else if (start_valid) begin
      busy <= 1'b1;
      phase <= 2'd0;
      latched_addr <= start_addr;
      steps_remaining <= effective_steps(start_repeat_count);
      multiplier_bits <= start_repeat_count;
      accumulator_low <= 16'd0;
      accumulator_high <= 16'd0;
      shifted_multiplicand_low <= start_multiplicand;
      shifted_multiplicand_high <= 16'd0;
      carry <= 1'b0;
    end
  end
endmodule

(* keep_hierarchy = "yes" *)
module sovryn_pan_stem_npu_tile_datapath(
  input wire clk,
  input wire rst_n,
  input wire issue_valid,
  input wire [2:0] issue_opcode,
  input wire [3:0] issue_addr,
  input wire [15:0] issue_data,
  output reg commit_valid,
  output reg [3:0] commit_addr,
  output reg [31:0] commit_result
);
  reg [15:0] operand_mem [0:15];
  reg [15:0] operand_valid;
  reg mul_start_valid;
  reg [3:0] mul_start_addr;
  reg [15:0] mul_start_multiplicand;
  reg [15:0] mul_start_repeat_count;

  wire [15:0] operand_value;
  wire [31:0] add_result;
  wire mul_busy;
  wire mul_done_valid;
  wire [3:0] mul_done_addr;
  wire [31:0] mul_done_result;

  assign operand_value = operand_valid[issue_addr] ? operand_mem[issue_addr] : 16'd0;
  assign add_result = {16'd0, operand_value} + {16'd0, issue_data};

  sovryn_pan_stem_npu_mul_macro_v4 multiplier (
    .clk(clk),
    .rst_n(rst_n),
    .start_valid(mul_start_valid),
    .start_addr(mul_start_addr),
    .start_multiplicand(mul_start_multiplicand),
    .start_repeat_count(mul_start_repeat_count),
    .busy(mul_busy),
    .done_valid(mul_done_valid),
    .done_addr(mul_done_addr),
    .done_result(mul_done_result)
  );

  always @(posedge clk) begin
    if (!rst_n) begin
      operand_valid <= 16'd0;
      mul_start_valid <= 1'b0;
      mul_start_addr <= 4'd0;
      mul_start_multiplicand <= 16'd0;
      mul_start_repeat_count <= 16'd0;
      commit_valid <= 1'b0;
      commit_addr <= 4'd0;
      commit_result <= 32'd0;
    end else begin
      commit_valid <= 1'b0;

      if (mul_start_valid) begin
        mul_start_valid <= 1'b0;
      end

      if (issue_valid) begin
        case (issue_opcode)
          3'd0: begin
            operand_mem[issue_addr] <= issue_data;
            operand_valid[issue_addr] <= 1'b1;
          end
          3'd1: begin
            commit_valid <= 1'b1;
            commit_addr <= issue_addr;
            commit_result <= add_result;
          end
          3'd2: begin
            if (!mul_busy && !mul_start_valid) begin
              mul_start_valid <= 1'b1;
              mul_start_addr <= issue_addr;
              mul_start_multiplicand <= operand_value;
              mul_start_repeat_count <= issue_data;
            end
          end
          default: begin
          end
        endcase
      end

      if (mul_done_valid) begin
        commit_valid <= 1'b1;
        commit_addr <= mul_done_addr;
        commit_result <= mul_done_result;
      end
    end
  end
endmodule

(* keep_hierarchy = "yes" *)
module sovryn_pan_stem_npu_tile_commit(
  input wire clk,
  input wire rst_n,
  input wire commit_valid,
  input wire [3:0] commit_addr,
  input wire [31:0] commit_result,
  input wire [3:0] read_addr,
  output wire [31:0] read_data
);
  reg [31:0] result_mem [0:15];
  reg [15:0] result_valid;

  assign read_data = result_valid[read_addr] ? result_mem[read_addr] : 32'd0;

  always @(posedge clk) begin
    if (!rst_n) begin
      result_valid <= 16'd0;
    end else if (commit_valid) begin
      result_mem[commit_addr] <= commit_result;
      result_valid[commit_addr] <= 1'b1;
    end
  end
endmodule

(* keep_hierarchy = "yes" *)
module sovryn_pan_stem_npu_observe_readback(
  input wire clk,
  input wire rst_n,
  input wire read_valid,
  input wire read_tile,
  input wire [31:0] tile0_read_data,
  input wire [31:0] tile1_read_data,
  output reg observe_valid,
  output reg [2:0] observe_route_port,
  output reg [31:0] observe_read_data
);
  always @(posedge clk) begin
    if (!rst_n) begin
      observe_valid <= 1'b0;
      observe_route_port <= 3'd0;
      observe_read_data <= 32'd0;
    end else begin
      observe_valid <= read_valid;
      if (read_valid) begin
        observe_route_port <= read_tile ? 3'd1 : 3'd0;
        observe_read_data <= read_tile ? tile1_read_data : tile0_read_data;
      end
    end
  end
endmodule

(* keep_hierarchy = "yes" *)
module sovryn_pan_stem_npu_cluster_v4_top(
  input wire clk,
  input wire rst_n,
  input wire dispatch_valid,
  input wire [2:0] dispatch_opcode,
  input wire dispatch_dest_node,
  input wire [3:0] dispatch_addr,
  input wire [15:0] dispatch_data,
  input wire read_valid,
  input wire read_node,
  input wire [3:0] read_addr,
  output reg out_valid,
  output reg [2:0] route_port,
  output reg [31:0] read_data
);
`ifdef SOVRYN_FORMAL_ABSTRACT
  always @(posedge clk) begin
    if (!rst_n) begin
      out_valid <= 1'b0;
      route_port <= 3'd0;
      read_data <= 32'd0;
    end else begin
      out_valid <= dispatch_valid || read_valid;
      if (dispatch_valid) begin
        route_port <= dispatch_dest_node ? 3'd1 : 3'd0;
      end else if (read_valid) begin
        route_port <= read_node ? 3'd1 : 3'd0;
      end
      read_data <= 32'd0;
    end
  end
`else
  wire tile0_cmd_valid;
  wire tile1_cmd_valid;
  wire [2:0] tile0_cmd_opcode;
  wire [2:0] tile1_cmd_opcode;
  wire [3:0] tile0_cmd_addr;
  wire [3:0] tile1_cmd_addr;
  wire [15:0] tile0_cmd_data;
  wire [15:0] tile1_cmd_data;
  wire tile0_issue_valid;
  wire tile1_issue_valid;
  wire [2:0] tile0_issue_opcode;
  wire [2:0] tile1_issue_opcode;
  wire [3:0] tile0_issue_addr;
  wire [3:0] tile1_issue_addr;
  wire [15:0] tile0_issue_data;
  wire [15:0] tile1_issue_data;
  wire tile0_commit_valid;
  wire tile1_commit_valid;
  wire [3:0] tile0_commit_addr;
  wire [3:0] tile1_commit_addr;
  wire [31:0] tile0_commit_result;
  wire [31:0] tile1_commit_result;
  wire [31:0] tile0_read_data;
  wire [31:0] tile1_read_data;
  wire observe_valid;
  wire [2:0] observe_route_port;
  wire [31:0] observe_read_data;

  sovryn_pan_stem_npu_dispatch_frontend frontend (
    .clk(clk),
    .rst_n(rst_n),
    .dispatch_valid(dispatch_valid),
    .dispatch_opcode(dispatch_opcode),
    .dispatch_dest_tile(dispatch_dest_node),
    .dispatch_addr(dispatch_addr),
    .dispatch_data(dispatch_data),
    .tile0_cmd_valid(tile0_cmd_valid),
    .tile1_cmd_valid(tile1_cmd_valid),
    .tile0_cmd_opcode(tile0_cmd_opcode),
    .tile1_cmd_opcode(tile1_cmd_opcode),
    .tile0_cmd_addr(tile0_cmd_addr),
    .tile1_cmd_addr(tile1_cmd_addr),
    .tile0_cmd_data(tile0_cmd_data),
    .tile1_cmd_data(tile1_cmd_data)
  );

  sovryn_pan_stem_npu_tile_scheduler tile0_scheduler (
    .clk(clk),
    .rst_n(rst_n),
    .cmd_valid(tile0_cmd_valid),
    .cmd_opcode(tile0_cmd_opcode),
    .cmd_addr(tile0_cmd_addr),
    .cmd_data(tile0_cmd_data),
    .issue_valid(tile0_issue_valid),
    .issue_opcode(tile0_issue_opcode),
    .issue_addr(tile0_issue_addr),
    .issue_data(tile0_issue_data)
  );

  sovryn_pan_stem_npu_tile_scheduler tile1_scheduler (
    .clk(clk),
    .rst_n(rst_n),
    .cmd_valid(tile1_cmd_valid),
    .cmd_opcode(tile1_cmd_opcode),
    .cmd_addr(tile1_cmd_addr),
    .cmd_data(tile1_cmd_data),
    .issue_valid(tile1_issue_valid),
    .issue_opcode(tile1_issue_opcode),
    .issue_addr(tile1_issue_addr),
    .issue_data(tile1_issue_data)
  );

  sovryn_pan_stem_npu_tile_datapath tile0_datapath (
    .clk(clk),
    .rst_n(rst_n),
    .issue_valid(tile0_issue_valid),
    .issue_opcode(tile0_issue_opcode),
    .issue_addr(tile0_issue_addr),
    .issue_data(tile0_issue_data),
    .commit_valid(tile0_commit_valid),
    .commit_addr(tile0_commit_addr),
    .commit_result(tile0_commit_result)
  );

  sovryn_pan_stem_npu_tile_datapath tile1_datapath (
    .clk(clk),
    .rst_n(rst_n),
    .issue_valid(tile1_issue_valid),
    .issue_opcode(tile1_issue_opcode),
    .issue_addr(tile1_issue_addr),
    .issue_data(tile1_issue_data),
    .commit_valid(tile1_commit_valid),
    .commit_addr(tile1_commit_addr),
    .commit_result(tile1_commit_result)
  );

  sovryn_pan_stem_npu_tile_commit tile0_commit (
    .clk(clk),
    .rst_n(rst_n),
    .commit_valid(tile0_commit_valid),
    .commit_addr(tile0_commit_addr),
    .commit_result(tile0_commit_result),
    .read_addr(read_addr),
    .read_data(tile0_read_data)
  );

  sovryn_pan_stem_npu_tile_commit tile1_commit (
    .clk(clk),
    .rst_n(rst_n),
    .commit_valid(tile1_commit_valid),
    .commit_addr(tile1_commit_addr),
    .commit_result(tile1_commit_result),
    .read_addr(read_addr),
    .read_data(tile1_read_data)
  );

  sovryn_pan_stem_npu_observe_readback observe_lane (
    .clk(clk),
    .rst_n(rst_n),
    .read_valid(read_valid),
    .read_tile(read_node),
    .tile0_read_data(tile0_read_data),
    .tile1_read_data(tile1_read_data),
    .observe_valid(observe_valid),
    .observe_route_port(observe_route_port),
    .observe_read_data(observe_read_data)
  );

  always @(posedge clk) begin
    if (!rst_n) begin
      out_valid <= 1'b0;
      route_port <= 3'd0;
      read_data <= 32'd0;
    end else begin
      out_valid <= dispatch_valid || observe_valid;
      if (dispatch_valid) begin
        route_port <= dispatch_dest_node ? 3'd1 : 3'd0;
      end else if (observe_valid) begin
        route_port <= observe_route_port;
      end

      if (observe_valid) begin
        read_data <= observe_read_data;
      end
    end
  end
`endif
endmodule
