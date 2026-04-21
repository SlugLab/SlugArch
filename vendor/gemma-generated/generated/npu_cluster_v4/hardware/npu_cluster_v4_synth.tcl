# Generated synthesis hook for npu_cluster_v4.
# Source this from Vivado/Quartus project scripts after setting the project.
set gemma_codegen_filelist [file normalize [file join [file dirname [info script]] npu_cluster_v4.f]]
set fp [open $gemma_codegen_filelist r]
set files [split [read $fp] "\n"]
close $fp
foreach f $files {
  set f [string trim $f]
  if {$f eq "" || [string match "#*" $f]} { continue }
  if {[file exists $f]} { read_verilog -sv $f }
}
set_property top gemma_codegen_npu_cluster_v4_df [current_fileset]
