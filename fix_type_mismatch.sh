#!/bin/bash
# 修复常见的类型不匹配错误

echo "修复类型不匹配错误..."

# 修复浮点数与整数比较
find src/widget -name "*.rs" -type f | while read file; do
    echo "检查文件: $file"
    
    # 检查是否有浮点数与整数比较
    if grep -q "== 0\|> 0\|< 0\|>= 0\|<= 0" "$file"; then
        echo "  -> 发现可能的类型不匹配"
        
        # 创建备份
        cp "$file" "$file.bak"
        
        # 修复浮点数比较
        sed -i -E '
            # 处理 f32/f64 与 0 比较
            s/([a-zA-Z_][a-zA-Z0-9_]*)\s*==\s*0(\.0)?/\1 == 0.0/g
            s/([a-zA-Z_][a-zA-Z0-9_]*)\s*>\s*0(\.0)?/\1 > 0.0/g
            s/([a-zA-Z_][a-zA-Z0-9_]*)\s*<\s*0(\.0)?/\1 < 0.0/g
            s/([a-zA-Z_][a-zA-Z0-9_]*)\s*>=\s*0(\.0)?/\1 >= 0.0/g
            s/([a-zA-Z_][a-zA-Z0-9_]*)\s*<=\s*0(\.0)?/\1 <= 0.0/g
            
            # 处理字面量浮点数
            s/:\s*([0-9]+)([^0-9\.])/: \1.0\2/g
        ' "$file"
        
        # 检查是否有变化
        if diff -q "$file" "$file.bak" > /dev/null; then
            rm "$file.bak"
            echo "  -> 无变化"
        else
            echo "  -> 已修复类型不匹配"
            rm "$file.bak"
        fi
    fi
done

echo "修复完成"