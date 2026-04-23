#!/bin/bash

# Rust Widgets API 迁移修复脚本
# 用于批量修复常见的API签名和类型不匹配问题

echo "开始修复 Rust Widgets API 迁移问题..."

# 1. 修复 fill_rect 调用
echo "修复 fill_rect 调用..."
find src -name "*.rs" -type f -exec sed -i 's/context\.fill_rect(\([^,]*\), \([^,]*\), \([^,]*\), \([^,]*\),/context.fill_rect(Rect::new(\1, \2, \3, \4),/g' {} \;

# 2. 修复 draw_rect 调用  
echo "修复 draw_rect 调用..."
find src -name "*.rs" -type f -exec sed -i 's/context\.draw_rect(\([^,]*\), \([^,]*\), \([^,]*\), \([^,]*\),/context.draw_rect(Rect::new(\1, \2, \3, \4),/g' {} \;

# 3. 修复简单的 draw_line 调用（单行格式）
echo "修复简单的 draw_line 调用..."
find src -name "*.rs" -type f -exec sed -i 's/context\.draw_line(\([^,]*\), \([^,]*\), \([^,]*\), \([^,]*\),/context.draw_line(Point::new(\1, \2), Point::new(\3, \4),/g' {} \;

# 4. 修复 draw_circle 调用
echo "修复 draw_circle 调用..."
find src -name "*.rs" -type f -exec sed -i 's/context\.draw_circle(\([^,]*\), \([^,]*\), \([^,]*\),/context.draw_circle(Point::new(\1, \2), \3 as u32,/g' {} \;

# 5. 修复 fill_circle 调用
echo "修复 fill_circle 调用..."
find src -name "*.rs" -type f -exec sed -i 's/context\.fill_circle(\([^,]*\), \([^,]*\), \([^,]*\),/context.fill_circle(Point::new(\1, \2), \3 as u32,/g' {} \;

# 6. 修复 i32 + u32 类型不匹配
echo "修复 i32 + u32 类型不匹配..."
find src -name "*.rs" -type f -exec sed -i 's/rect\.x + rect\.width/rect.x + rect.width as i32/g' {} \;
find src -name "*.rs" -type f -exec sed -i 's/rect\.y + rect\.height/rect.y + rect.height as i32/g' {} \;

# 7. 修复 u32 / f64 类型不匹配
echo "修复 u32 / f64 类型不匹配..."
find src -name "*.rs" -type f -exec sed -i 's/rect\.width \/ 2\.0/rect.width as i32 \/ 2/g' {} \;
find src -name "*.rs" -type f -exec sed -i 's/rect\.height \/ 2\.0/rect.height as i32 \/ 2/g' {} \;

# 8. 修复 draw_text 调用（移除对齐参数）
echo "修复 draw_text 调用..."
find src -name "*.rs" -type f -exec sed -i 's/context\.draw_text(\([^,]*\), \([^,]*\), \([^,]*\), \([^,]*\), \([^,]*\),/context.draw_text(Point::new(\1, \2), \3, \4,/g' {} \;

echo "修复完成！请运行 'cargo check --lib' 验证修复结果。"