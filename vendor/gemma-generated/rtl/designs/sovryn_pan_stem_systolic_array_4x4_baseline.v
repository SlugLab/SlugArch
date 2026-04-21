module sovryn_pan_stem_systolic_array_4x4(
  input wire clk,
  input wire rst_n,
  input wire in_valid,
  input wire [7:0] a00,
  input wire [7:0] a01,
  input wire [7:0] a02,
  input wire [7:0] a03,
  input wire [7:0] a10,
  input wire [7:0] a11,
  input wire [7:0] a12,
  input wire [7:0] a13,
  input wire [7:0] a20,
  input wire [7:0] a21,
  input wire [7:0] a22,
  input wire [7:0] a23,
  input wire [7:0] a30,
  input wire [7:0] a31,
  input wire [7:0] a32,
  input wire [7:0] a33,
  input wire [7:0] b00,
  input wire [7:0] b01,
  input wire [7:0] b02,
  input wire [7:0] b03,
  input wire [7:0] b10,
  input wire [7:0] b11,
  input wire [7:0] b12,
  input wire [7:0] b13,
  input wire [7:0] b20,
  input wire [7:0] b21,
  input wire [7:0] b22,
  input wire [7:0] b23,
  input wire [7:0] b30,
  input wire [7:0] b31,
  input wire [7:0] b32,
  input wire [7:0] b33,
  output reg out_valid,
  output reg [17:0] c00,
  output reg [17:0] c01,
  output reg [17:0] c02,
  output reg [17:0] c03,
  output reg [17:0] c10,
  output reg [17:0] c11,
  output reg [17:0] c12,
  output reg [17:0] c13,
  output reg [17:0] c20,
  output reg [17:0] c21,
  output reg [17:0] c22,
  output reg [17:0] c23,
  output reg [17:0] c30,
  output reg [17:0] c31,
  output reg [17:0] c32,
  output reg [17:0] c33
);
  reg [7:0] a00_pipe; reg [7:0] a01_pipe; reg [7:0] a02_pipe; reg [7:0] a03_pipe;
  reg [7:0] a10_pipe; reg [7:0] a11_pipe; reg [7:0] a12_pipe; reg [7:0] a13_pipe;
  reg [7:0] a20_pipe; reg [7:0] a21_pipe; reg [7:0] a22_pipe; reg [7:0] a23_pipe;
  reg [7:0] a30_pipe; reg [7:0] a31_pipe; reg [7:0] a32_pipe; reg [7:0] a33_pipe;
  reg [7:0] b00_pipe; reg [7:0] b01_pipe; reg [7:0] b02_pipe; reg [7:0] b03_pipe;
  reg [7:0] b10_pipe; reg [7:0] b11_pipe; reg [7:0] b12_pipe; reg [7:0] b13_pipe;
  reg [7:0] b20_pipe; reg [7:0] b21_pipe; reg [7:0] b22_pipe; reg [7:0] b23_pipe;
  reg [7:0] b30_pipe; reg [7:0] b31_pipe; reg [7:0] b32_pipe; reg [7:0] b33_pipe;
  reg valid_pipe;

  always @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
      a00_pipe <= 8'd0; a01_pipe <= 8'd0; a02_pipe <= 8'd0; a03_pipe <= 8'd0;
      a10_pipe <= 8'd0; a11_pipe <= 8'd0; a12_pipe <= 8'd0; a13_pipe <= 8'd0;
      a20_pipe <= 8'd0; a21_pipe <= 8'd0; a22_pipe <= 8'd0; a23_pipe <= 8'd0;
      a30_pipe <= 8'd0; a31_pipe <= 8'd0; a32_pipe <= 8'd0; a33_pipe <= 8'd0;
      b00_pipe <= 8'd0; b01_pipe <= 8'd0; b02_pipe <= 8'd0; b03_pipe <= 8'd0;
      b10_pipe <= 8'd0; b11_pipe <= 8'd0; b12_pipe <= 8'd0; b13_pipe <= 8'd0;
      b20_pipe <= 8'd0; b21_pipe <= 8'd0; b22_pipe <= 8'd0; b23_pipe <= 8'd0;
      b30_pipe <= 8'd0; b31_pipe <= 8'd0; b32_pipe <= 8'd0; b33_pipe <= 8'd0;
      valid_pipe <= 1'b0;
      out_valid <= 1'b0;
      c00 <= 18'd0; c01 <= 18'd0; c02 <= 18'd0; c03 <= 18'd0;
      c10 <= 18'd0; c11 <= 18'd0; c12 <= 18'd0; c13 <= 18'd0;
      c20 <= 18'd0; c21 <= 18'd0; c22 <= 18'd0; c23 <= 18'd0;
      c30 <= 18'd0; c31 <= 18'd0; c32 <= 18'd0; c33 <= 18'd0;
    end else begin
      a00_pipe <= a00; a01_pipe <= a01; a02_pipe <= a02; a03_pipe <= a03;
      a10_pipe <= a10; a11_pipe <= a11; a12_pipe <= a12; a13_pipe <= a13;
      a20_pipe <= a20; a21_pipe <= a21; a22_pipe <= a22; a23_pipe <= a23;
      a30_pipe <= a30; a31_pipe <= a31; a32_pipe <= a32; a33_pipe <= a33;
      b00_pipe <= b00; b01_pipe <= b01; b02_pipe <= b02; b03_pipe <= b03;
      b10_pipe <= b10; b11_pipe <= b11; b12_pipe <= b12; b13_pipe <= b13;
      b20_pipe <= b20; b21_pipe <= b21; b22_pipe <= b22; b23_pipe <= b23;
      b30_pipe <= b30; b31_pipe <= b31; b32_pipe <= b32; b33_pipe <= b33;
      valid_pipe <= in_valid;
      out_valid <= valid_pipe;
      c00 <= ({10'd0, a00_pipe} * {10'd0, b00_pipe}) + ({10'd0, a01_pipe} * {10'd0, b10_pipe}) + ({10'd0, a02_pipe} * {10'd0, b20_pipe}) + ({10'd0, a03_pipe} * {10'd0, b30_pipe});
      c01 <= ({10'd0, a00_pipe} * {10'd0, b01_pipe}) + ({10'd0, a01_pipe} * {10'd0, b11_pipe}) + ({10'd0, a02_pipe} * {10'd0, b21_pipe}) + ({10'd0, a03_pipe} * {10'd0, b31_pipe});
      c02 <= ({10'd0, a00_pipe} * {10'd0, b02_pipe}) + ({10'd0, a01_pipe} * {10'd0, b12_pipe}) + ({10'd0, a02_pipe} * {10'd0, b22_pipe}) + ({10'd0, a03_pipe} * {10'd0, b32_pipe});
      c03 <= ({10'd0, a00_pipe} * {10'd0, b03_pipe}) + ({10'd0, a01_pipe} * {10'd0, b13_pipe}) + ({10'd0, a02_pipe} * {10'd0, b23_pipe}) + ({10'd0, a03_pipe} * {10'd0, b33_pipe});
      c10 <= ({10'd0, a10_pipe} * {10'd0, b00_pipe}) + ({10'd0, a11_pipe} * {10'd0, b10_pipe}) + ({10'd0, a12_pipe} * {10'd0, b20_pipe}) + ({10'd0, a13_pipe} * {10'd0, b30_pipe});
      c11 <= ({10'd0, a10_pipe} * {10'd0, b01_pipe}) + ({10'd0, a11_pipe} * {10'd0, b11_pipe}) + ({10'd0, a12_pipe} * {10'd0, b21_pipe}) + ({10'd0, a13_pipe} * {10'd0, b31_pipe});
      c12 <= ({10'd0, a10_pipe} * {10'd0, b02_pipe}) + ({10'd0, a11_pipe} * {10'd0, b12_pipe}) + ({10'd0, a12_pipe} * {10'd0, b22_pipe}) + ({10'd0, a13_pipe} * {10'd0, b32_pipe});
      c13 <= ({10'd0, a10_pipe} * {10'd0, b03_pipe}) + ({10'd0, a11_pipe} * {10'd0, b13_pipe}) + ({10'd0, a12_pipe} * {10'd0, b23_pipe}) + ({10'd0, a13_pipe} * {10'd0, b33_pipe});
      c20 <= ({10'd0, a20_pipe} * {10'd0, b00_pipe}) + ({10'd0, a21_pipe} * {10'd0, b10_pipe}) + ({10'd0, a22_pipe} * {10'd0, b20_pipe}) + ({10'd0, a23_pipe} * {10'd0, b30_pipe});
      c21 <= ({10'd0, a20_pipe} * {10'd0, b01_pipe}) + ({10'd0, a21_pipe} * {10'd0, b11_pipe}) + ({10'd0, a22_pipe} * {10'd0, b21_pipe}) + ({10'd0, a23_pipe} * {10'd0, b31_pipe});
      c22 <= ({10'd0, a20_pipe} * {10'd0, b02_pipe}) + ({10'd0, a21_pipe} * {10'd0, b12_pipe}) + ({10'd0, a22_pipe} * {10'd0, b22_pipe}) + ({10'd0, a23_pipe} * {10'd0, b32_pipe});
      c23 <= ({10'd0, a20_pipe} * {10'd0, b03_pipe}) + ({10'd0, a21_pipe} * {10'd0, b13_pipe}) + ({10'd0, a22_pipe} * {10'd0, b23_pipe}) + ({10'd0, a23_pipe} * {10'd0, b33_pipe});
      c30 <= ({10'd0, a30_pipe} * {10'd0, b00_pipe}) + ({10'd0, a31_pipe} * {10'd0, b10_pipe}) + ({10'd0, a32_pipe} * {10'd0, b20_pipe}) + ({10'd0, a33_pipe} * {10'd0, b30_pipe});
      c31 <= ({10'd0, a30_pipe} * {10'd0, b01_pipe}) + ({10'd0, a31_pipe} * {10'd0, b11_pipe}) + ({10'd0, a32_pipe} * {10'd0, b21_pipe}) + ({10'd0, a33_pipe} * {10'd0, b31_pipe});
      c32 <= ({10'd0, a30_pipe} * {10'd0, b02_pipe}) + ({10'd0, a31_pipe} * {10'd0, b12_pipe}) + ({10'd0, a32_pipe} * {10'd0, b22_pipe}) + ({10'd0, a33_pipe} * {10'd0, b32_pipe});
      c33 <= ({10'd0, a30_pipe} * {10'd0, b03_pipe}) + ({10'd0, a31_pipe} * {10'd0, b13_pipe}) + ({10'd0, a32_pipe} * {10'd0, b23_pipe}) + ({10'd0, a33_pipe} * {10'd0, b33_pipe});
    end
  end
endmodule
