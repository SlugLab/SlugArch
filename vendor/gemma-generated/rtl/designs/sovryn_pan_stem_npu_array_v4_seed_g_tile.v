module sovryn_pan_stem_npu_array_v4_seed_g_tile(
  input wire clk,
  input wire rst_n,
  input wire advance_tick,
  input wire signed [15:0] in_north,
  input wire signed [15:0] in_west,
  output wire signed [15:0] out_south,
  output wire signed [15:0] out_east,
  output wire boundary_live
);
  wire step_valid;
  wire [15:0] current_instruction;

  sovryn_pan_stem_npu_array_v4_seed_g_issue issue_inst (
    .clk(clk),
    .rst_n(rst_n),
    .advance_tick(advance_tick),
    .current_instruction(current_instruction),
    .step_valid(step_valid)
  );

  sovryn_pan_stem_npu_array_v4_seed_g_state state_inst (
    .clk(clk),
    .rst_n(rst_n),
    .step_valid(step_valid),
    .current_instruction(current_instruction),
    .in_north(in_north),
    .in_west(in_west),
    .out_south(out_south),
    .out_east(out_east),
    .boundary_live(boundary_live)
  );
endmodule
