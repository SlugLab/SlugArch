// Generated smoke test for systolic_array_32x32.
`timescale 1ns/1ps

module gemma_codegen_systolic_array_32x32_df_tb;
  localparam integer TOKEN_WIDTH = 256;
  reg clk;
  reg rst_n;
  reg cmd_valid;
  wire cmd_ready;
  reg [TOKEN_WIDTH-1:0] token_in;
  wire done_valid;
  wire [TOKEN_WIDTH-1:0] token_out;

  gemma_codegen_systolic_array_32x32_df #(.TOKEN_WIDTH(TOKEN_WIDTH)) dut (
    .clk(clk),
    .rst_n(rst_n),
    .cmd_valid(cmd_valid),
    .cmd_ready(cmd_ready),
    .token_in(token_in),
    .done_valid(done_valid),
    .token_out(token_out)
  );

  initial clk = 1'b0;
  always #5 clk = ~clk;

  initial begin
    rst_n = 1'b0;
    cmd_valid = 1'b0;
    token_in = {TOKEN_WIDTH{1'b0}};
    repeat (4) @(posedge clk);
    rst_n = 1'b1;
    @(posedge clk);
    token_in = {TOKEN_WIDTH/32{32'h1357_2468}};
    cmd_valid = 1'b1;
    @(posedge clk);
    cmd_valid = 1'b0;
    repeat (16) @(posedge clk);
    $display("systolic_array_32x32 done_valid=%0d token_out[31:0]=0x%08x", done_valid, token_out[31:0]);
    $finish;
  end
endmodule
