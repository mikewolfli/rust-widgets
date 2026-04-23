#!/usr/bin/env python3
import re
import os

def fix_draw_line_in_file(filepath):
    """修复文件中的draw_line调用"""
    with open(filepath, 'r') as f:
        content = f.read()
    
    # 模式1: 单行draw_line调用
    pattern1 = r'context\.draw_line\(([^,]+),\s*([^,]+),\s*([^,]+),\s*([^,]+),\s*([^)]+)\)'
    
    def replace_match(match):
        x1 = match.group(1).strip()
        y1 = match.group(2).strip()
        x2 = match.group(3).strip()
        y2 = match.group(4).strip()
        color = match.group(5).strip()
        
        # 处理line_width类型转换
        x2_fixed = re.sub(r' - line_width$', r' - line_width as i32', x2)
        y2_fixed = re.sub(r' - line_width$', r' - line_width as i32', y2)
        
        return f'context.draw_line(Point::new({x1}, {y1}), Point::new({x2_fixed}, {y2_fixed}), {color})'
    
    new_content = re.sub(pattern1, replace_match, content)
    
    # 模式2: 多行draw_line调用（简化处理）
    # 查找所有draw_line调用
    lines = new_content.split('\n')
    in_draw_line = False
    draw_line_start = 0
    params = []
    
    for i, line in enumerate(lines):
        if 'context.draw_line(' in line and not 'Point::new' in line:
            in_draw_line = True
            draw_line_start = i
            # 提取第一行的参数
            line_part = line[line.find('context.draw_line(')+18:]
            params = [p.strip() for p in line_part.split(',') if p.strip()]
        
        elif in_draw_line and ')' in line:
            # 收集剩余参数
            line_before_paren = line.split(')')[0]
            if line_before_paren.strip():
                params.extend([p.strip() for p in line_before_paren.split(',') if p.strip()])
            
            if len(params) >= 5:
                # 重建调用
                x1, y1, x2, y2, color = params[0], params[1], params[2], params[3], params[4]
                
                # 处理line_width类型转换
                x2_fixed = re.sub(r' - line_width$', r' - line_width as i32', x2)
                y2_fixed = re.sub(r' - line_width$', r' - line_width as i32', y2)
                
                new_call = f'context.draw_line(Point::new({x1}, {y1}), Point::new({x2_fixed}, {y2_fixed}), {color})'
                
                # 替换多行
                lines[draw_line_start] = new_call
                for j in range(draw_line_start+1, i+1):
                    lines[j] = ''
            
            in_draw_line = False
            params = []
        
        elif in_draw_line:
            # 收集参数
            params.extend([p.strip() for p in line.split(',') if p.strip()])
    
    new_content = '\n'.join(lines)
    
    # 清理空行
    new_content = re.sub(r'\n\s*\n', '\n', new_content)
    
    if new_content != content:
        with open(filepath, 'w') as f:
            f.write(new_content)
        print(f"修复了 {filepath}")
        return True
    
    return False

def main():
    print("修复draw_line调用...")
    
    # 处理所有Rust文件
    for root, dirs, files in os.walk('src'):
        for file in files:
            if file.endswith('.rs'):
                filepath = os.path.join(root, file)
                fix_draw_line_in_file(filepath)
    
    print("修复完成")

if __name__ == '__main__':
    main()