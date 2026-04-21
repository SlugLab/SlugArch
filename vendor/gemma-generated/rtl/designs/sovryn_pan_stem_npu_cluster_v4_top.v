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
