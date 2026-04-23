#!/bin/bash
# 修复Color::from_rgb调用

echo "修复Color::from_rgb调用..."

# 修复Color::from_rgb调用：从1个参数改为3个参数
find src/widget -name "*.rs" -type f | while read file; do
    echo "检查文件: $file"
    
    # 检查是否有Color::from_rgb(数字)调用
    if grep -q "Color::from_rgb([0-9][0-9]*)" "$file"; then
        echo "  -> 发现Color::from_rgb调用"
        
        # 创建备份
        cp "$file" "$file.bak"
        
        # 修复Color::from_rgb调用
        sed -i -E '
            # 处理Color::from_rgb(数字) -> Color::from_rgb(数字, 数字, 数字)
            s/Color::from_rgb\(([0-9][0-9]*)\)/Color::from_rgb(\1, \1, \1)/g
            s/Color::from_rgb\(([0-9][0-9]*)\s*as\s*u32\)/Color::from_rgb(\1, \1, \1)/g
        ' "$file"
        
        # 检查是否有变化
        if diff -q "$file" "$file.bak" > /dev/null; then
            rm "$file.bak"
            echo "  -> 无变化"
        else
            echo "  -> 已修复Color::from_rgb调用"
            rm "$file.bak"
        fi
    fi
done

echo "修复完成"