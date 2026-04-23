#!/bin/bash

# 修复浮点数运算错误

echo "修复浮点数运算错误..."

# 修复常见的浮点数运算模式
for file in src/widget/**/*.rs; do
    if [[ -f "$file" ]]; then
        echo "处理文件: $file"
        
        # 修复浮点数字面量
        sed -i 's/\([a-zA-Z_][a-zA-Z0-9_]*\)\.x + \([0-9]\+\)\.0/\1.x + \2/g' "$file"
        sed -i 's/\([a-zA-Z_][a-zA-Z0-9_]*\)\.y + \([0-9]\+\)\.0/\1.y + \2/g' "$file"
        sed -i 's/\([a-zA-Z_][a-zA-Z0-9_]*\)\.width + \([0-9]\+\)\.0/\1.width + \2/g' "$file"
        sed -i 's/\([a-zA-Z_][a-zA-Z0-9_]*\)\.height + \([0-9]\+\)\.0/\1.height + \2/g' "$file"
        
        sed -i 's/\([a-zA-Z_][a-zA-Z0-9_]*\)\.x - \([0-9]\+\)\.0/\1.x - \2/g' "$file"
        sed -i 's/\([a-zA-Z_][a-zA-Z0-9_]*\)\.y - \([0-9]\+\)\.0/\1.y - \2/g' "$file"
        sed -i 's/\([a-zA-Z_][a-zA-Z0-9_]*\)\.width - \([0-9]\+\)\.0/\1.width - \2/g' "$file"
        sed -i 's/\([a-zA-Z_][a-zA-Z0-9_]*\)\.height - \([0-9]\+\)\.0/\1.height - \2/g' "$file"
        
        # 修复除法运算
        sed -i 's/\([a-zA-Z_][a-zA-Z0-9_]*\)\.width \/ 2\.0/\1.width as i32 \/ 2/g' "$file"
        sed -i 's/\([a-zA-Z_][a-zA-Z0-9_]*\)\.height \/ 2\.0/\1.height as i32 \/ 2/g' "$file"
        sed -i 's/\([a-zA-Z_][a-zA-Z0-9_]*\)\.width \/ 4\.0/\1.width as i32 \/ 4/g' "$file"
        sed -i 's/\([a-zA-Z_][a-zA-Z0-9_]*\)\.height \/ 4\.0/\1.height as i32 \/ 4/g' "$file"
        
        # 修复浮点数乘法
        sed -i 's/\([a-zA-Z_][a-zA-Z0-9_]*\)\.width \* 0\.3/\1.width \* 3 \/ 10/g' "$file"
        sed -i 's/\([a-zA-Z_][a-zA-Z0-9_]*\)\.height \* 0\.3/\1.height \* 3 \/ 10/g' "$file"
        sed -i 's/\([a-zA-Z_][a-zA-Z0-9_]*\)\.width \* 0\.5/\1.width \/ 2/g' "$file"
        sed -i 's/\([a-zA-Z_][a-zA-Z0-9_]*\)\.height \* 0\.5/\1.height \/ 2/g' "$file"
        
        # 修复 line_width 相关的运算
        sed -i 's/\([a-zA-Z_][a-zA-Z0-9_]*\)\.x + line_width/\1.x + line_width as i32/g' "$file"
        sed -i 's/\([a-zA-Z_][a-zA-Z0-9_]*\)\.y + line_width/\1.y + line_width as i32/g' "$file"
        sed -i 's/\([a-zA-Z_][a-zA-Z0-9_]*\)\.width - line_width/\1.width - line_width as u32/g' "$file"
        sed -i 's/\([a-zA-Z_][a-zA-Z0-9_]*\)\.height - line_width/\1.height - line_width as u32/g' "$file"
        
        # 修复简单的浮点数字面量
        sed -i 's/\b\([0-9]\+\)\.0\b/\1/g' "$file"
        sed -i 's/\b\([0-9]\+\)\.0f32\b/\1/g' "$file"
        sed -i 's/\b\([0-9]\+\)\.0f64\b/\1/g' "$file"
    fi
done

echo "浮点数运算修复完成"