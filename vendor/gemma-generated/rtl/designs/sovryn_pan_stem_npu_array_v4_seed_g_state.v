module sovryn_pan_stem_npu_array_v4_seed_g_state(
  input wire clk,
  input wire rst_n,
  input wire step_valid,
  input wire [15:0] current_instruction,
  input wire signed [15:0] in_north,
  input wire signed [15:0] in_west,
  output reg signed [15:0] out_south,
  output reg signed [15:0] out_east,
  output wire boundary_live
);
  reg signed [15:0] accumulator_q;

  assign boundary_live = accumulator_q[0] ^ out_south[0];

  always @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
      accumulator_q <= 16'd0;
      out_south <= 16'd0;
      out_east <= 16'd0;
    end else if (step_valid) begin
      // Decode instruction
      // Opcode: [15:12], SrcA: [11:10], SrcB: [9:8], Weight: [7:0]
      // (ignoring full parameterized decode for brevity of the bootstrap)
      automatic logic [3:0] opcode = current_instruction[15:12];
      automatic logic [1:0] src_a = current_instruction[11:10];
      automatic logic [1:0] src_b = current_instruction[9:8];
      automatic logic signed [7:0] weight = current_instruction[7:0];
      
      automatic logic signed [15:0] val_a = (src_a == 2'd0) ? in_north :
                                            (src_a == 2'd1) ? in_west :
                                            accumulator_q;

      automatic logic signed [15:0] val_b = (src_b == 2'd0) ? in_north :
                                            (src_b == 2'd1) ? in_west :
                                            accumulator_q;

      automatic logic signed [15:0] result;

      // NOP=0, MAC=1, ADD=2, RELU=3
      case (opcode)
        4'd1: result = (val_a * weight) + val_b; // MAC
        4'd2: result = val_a + val_b;             // ADD
        4'd3: result = (val_a > 0) ? val_a : 16'd0; // RELU
        default: result = val_a;                  // NOP 
      endcase

      accumulator_q <= result;
      
      // Output routing: by default send out the result south and east
      out_south <= result;
      out_east <= result;
    end
  end
endmodule
