#!/bin/bash
# 专门修复frame.rs文件

echo "修复frame.rs文件..."

file="src/widget/base_widgets/frame.rs"
cp "$file" "$file.bak"

# 修复所有有问题的draw_line调用
sed -i -E '
    # 修复嵌套的Point::new
    s/Point::new\(Point::new\(([^)]+)\)\)/Point::new(\1)/g
    
    # 修复缺少逗号的draw_line调用
    s/draw_line\(Point::new\(([^)]+)\), Point::new\(([^)]+)\), ([^,]+),\)/draw_line(Point::new(\1), Point::new(\2), \3);/g
    
    # 修复类型转换问题
    s/line_width as i32 as i32 as i32/line_width as i32/g
    s/rect.height as i32 as i32/rect.height as i32/g
    s/rect.width as i32 as i32/rect.width as i32/g
    
    # 修复浮点数比较
    s/mid_line_width > 0/mid_line_width > 0.0/g
' "$file"

# 检查修复结果
if diff -q "$file" "$file.bak" > /dev/null; then
    echo "无变化"
else
    echo "已修复frame.rs"
    # 显示修复的差异
    diff -u "$file.bak" "$file" | head -50
fi

rm "$file.bak"
echo "修复完成"