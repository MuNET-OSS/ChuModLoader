fn main() {
    // d3d9.dll 的关键导出只有 Direct3DCreate9，由 lib.rs 手动实现
    // 不使用 forward-dll，避免跟手动导出冲突
}
