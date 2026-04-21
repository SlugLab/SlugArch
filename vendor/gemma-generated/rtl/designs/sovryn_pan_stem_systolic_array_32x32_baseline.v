module sovryn_pan_stem_systolic_array_32x32(
  input wire clk,
  input wire rst_n,
  input wire load_valid,
  input wire load_matrix_sel,
  input wire [9:0] load_addr,
  input wire [7:0] load_data,
  input wire compute_valid,
  input wire read_valid,
  input wire [9:0] read_addr,
  output reg out_valid,
  output reg [23:0] read_data
);
  integer row;
  integer col;
  integer k;
  integer idx;
  (* nomem2reg *) reg [7:0] a_mem [0:1023];
  (* nomem2reg *) reg [7:0] b_mem [0:1023];
  (* nomem2reg *) reg [23:0] c_mem [0:1023];
  reg [31:0] sum_acc;

  always @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
      out_valid <= 1'b0;
      read_data <= 24'd0;
      for (idx = 0; idx < 1024; idx = idx + 1) begin
        a_mem[idx] = 8'd0;
        b_mem[idx] = 8'd0;
        c_mem[idx] = 24'd0;
      end
    end else begin
      out_valid <= 1'b0;
      if (load_valid) begin
        if (load_matrix_sel) begin
          b_mem[load_addr] <= load_data;
        end else begin
          a_mem[load_addr] <= load_data;
        end
      end
      if (compute_valid) begin
        for (row = 0; row < 32; row = row + 1) begin
          for (col = 0; col < 32; col = col + 1) begin
            sum_acc = 32'd0;
            for (k = 0; k < 32; k = k + 1) begin
              sum_acc = sum_acc + ({24'd0, a_mem[(row * 32) + k]} * {24'd0, b_mem[(k * 32) + col]});
            end
            c_mem[(row * 32) + col] = sum_acc[23:0];
          end
        end
        out_valid <= 1'b1;
      end else if (read_valid) begin
        out_valid <= 1'b1;
        read_data <= c_mem[read_addr];
      end
    end
  end
endmodule
