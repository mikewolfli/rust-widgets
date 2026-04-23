#!/bin/bash

# 修复 draw_text 调用（移除对齐参数）

echo "修复 draw_text 调用..."

# 修复常见的 draw_text 调用模式
for file in src/widget/**/*.rs; do
    if [[ -f "$file" ]]; then
        echo "处理文件: $file"
        
        # 模式1: context.draw_text(x, y, text, font, color, alignment)
        sed -i 's/context\.draw_text([[:space:]]*\([a-zA-Z_][a-zA-Z0-9_]*\)\.x,[[:space:]]*\([a-zA-Z_][a-zA-Z0-9_]*\)\.y,[[:space:]]*\([^,]*\),[[:space:]]*\([^,]*\),[[:space:]]*\([^,]*\),[[:space:]]*\([a-zA-Z_][a-zA-Z0-9_]*\)/context.draw_text(Point::new(\1.x, \1.y), \3, \4, \5/g' "$file"
        
        # 模式2: context.draw_text(x, y, &text, &font, color, alignment)
        sed -i 's/context\.draw_text([[:space:]]*\([a-zA-Z_][a-zA-Z0-9_]*\),[[:space:]]*\([a-zA-Z_][a-zA-Z0-9_]*\),[[:space:]]*&\([a-zA-Z_][a-zA-Z0-9_]*\),[[:space:]]*&\([a-zA-Z_][a-zA-Z0-9_]*\)(\([^)]*\)),[[:space:]]*\([a-zA-Z_][a-zA-Z0-9_]*\)(\([^)]*\)),[[:space:]]*\([a-zA-Z_][a-zA-Z0-9_]*\)/context.draw_text(Point::new(\1, \2), \&\3, \&\4(\5), \6(\7)/g' "$file"
        
        # 模式3: context.draw_text(x + offset, y + offset, text, font, color, alignment)
        sed -i 's/context\.draw_text([[:space:]]*\([a-zA-Z_][a-zA-Z0-9_]*\)\.x + \([0-9]\+\)\.0,[[:space:]]*\([a-zA-Z_][a-zA-Z0-9_]*\)\.y + \([0-9]\+\)\.0,[[:space:]]*\([^,]*\),[[:space:]]*\([^,]*\),[[:space:]]*\([^,]*\),[[:space:]]*\([a-zA-Z_][a-zA-Z0-9_]*\)/context.draw_text(Point::new(\1.x + \2, \1.y + \4), \5, \6, \7/g' "$file"
        
        # 模式4: context.draw_text(x, y, &self.title, &Font::default(), color, alignment)
        sed -i 's/context\.draw_text([[:space:]]*\([a-zA-Z_][a-zA-Z0-9_]*\),[[:space:]]*\([a-zA-Z_][a-zA-Z0-9_]*\),[[:space:]]*&self\.title,[[:space:]]*&Font::default(),[[:space:]]*\([a-zA-Z_][a-zA-Z0-9_]*\)(\([^)]*\)),[[:space:]]*\([a-zA-Z_][a-zA-Z0-9_]*\)/context.draw_text(Point::new(\1, \2), \&self.title, \&Font::default(), \3(\4))/g' "$file"
        
        # 模式5: context.draw_text(x, y, text, &Font::default(), color, alignment)
        sed -i 's/context\.draw_text([[:space:]]*\([a-zA-Z_][a-zA-Z0-9_]*\),[[:space:]]*\([a-zA-Z_][a-zA-Z0-9_]*\),[[:space:]]*\([^,]*\),[[:space:]]*&Font::default(),[[:space:]]*\([a-zA-Z_][a-zA-Z0-9_]*\)(\([^)]*\)),[[:space:]]*\([a-zA-Z_][a-zA-Z0-9_]*\)/context.draw_text(Point::new(\1, \2), \3, \&Font::default(), \4(\5))/g' "$file"
    fi
done

echo "draw_text 调用修复完成"