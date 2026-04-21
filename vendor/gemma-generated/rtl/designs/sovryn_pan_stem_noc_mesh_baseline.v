module sovryn_pan_stem_noc_mesh(
  input wire clk,
  input wire rst_n,
  input wire in_valid,
  input wire credit_ready,
  input wire [1:0] current_x,
  input wire [1:0] current_y,
  input wire [1:0] dest_x,
  input wire [1:0] dest_y,
  input wire [31:0] data_in,
  output reg out_valid,
  output reg delivered,
  output reg blocked,
  output reg [2:0] next_port,
  output reg [31:0] data_out
);
  always @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
      out_valid <= 1'b0;
      delivered <= 1'b0;
      blocked <= 1'b0;
      next_port <= 3'd0;
      data_out <= 32'd0;
    end else begin
      out_valid <= 1'b0;
      delivered <= 1'b0;
      blocked <= 1'b0;
      if (in_valid) begin
        data_out <= data_in;
        if (!credit_ready) begin
          blocked <= 1'b1;
          next_port <= 3'd0;
        end else if (current_x != dest_x) begin
          out_valid <= 1'b1;
          next_port <= dest_x > current_x ? 3'd1 : 3'd2;
        end else if (current_y != dest_y) begin
          out_valid <= 1'b1;
          next_port <= dest_y > current_y ? 3'd3 : 3'd4;
        end else begin
          out_valid <= 1'b1;
          delivered <= 1'b1;
          next_port <= 3'd0;
        end
      end
    end
  end
endmodule
