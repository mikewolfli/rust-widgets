#!/bin/bash

# 修复 BaseWidget.draw() 调用
# 这些调用应该被移除，因为 BaseWidget 没有 draw 方法

echo "修复 BaseWidget.draw() 调用..."

# 查找并移除 self.base.draw(context) 调用
find src -name "*.rs" -type f -exec sed -i '/self\.base\.draw(context);/d' {} \;

# 查找并移除 self.base_mut().draw(context) 调用  
find src -name "*.rs" -type f -exec sed -i '/self\.base_mut()\.draw(context);/d' {} \;

# 查找并移除 base.draw(context) 调用
find src -name "*.rs" -type f -exec sed -i '/base\.draw(context);/d' {} \;

echo "修复完成！"