#!/bin/bash

# 修复浮点数与整数运算错误

echo "修复浮点数与整数运算错误..."

# 修复 rect.x + line_width 模式（line_width 是 f32）
find src -name "*.rs" -type f -exec sed -i 's/rect\.x + line_width/rect.x + line_width as i32/g' {} \;
find src -name "*.rs" -type f -exec sed -i 's/rect\.y + line_width/rect.y + line_width as i32/g' {} \;

# 修复 rect.width - line_width 模式
find src -name "*.rs" -type f -exec sed -i 's/rect\.width - line_width/rect.width as i32 - line_width as i32/g' {} \;
find src -name "*.rs" -type f -exec sed -i 's/rect\.height - line_width/rect.height as i32 - line_width as i32/g' {} \;

# 修复其他常见的浮点数运算
find src -name "*.rs" -type f -exec sed -i 's/\([a-zA-Z_][a-zA-Z0-9_]*\)\.cos() as f32/\1.cos() as f32/g' {} \;
find src -name "*.rs" -type f -exec sed -i 's/\([a-zA-Z_][a-zA-Z0-9_]*\)\.sin() as f32/\1.sin() as f32/g' {} \;

# 修复 f32 * f64 乘法
find src -name "*.rs" -type f -exec sed -i 's/as f32 \* \([a-zA-Z_][a-zA-Z0-9_]*\)\.cos()/as f32 * \1.cos() as f32/g' {} \;
find src -name "*.rs" -type f -exec sed -i 's/as f32 \* \([a-zA-Z_][a-zA-Z0-9_]*\)\.sin()/as f32 * \1.sin() as f32/g' {} \;

echo "修复完成！"