#!/bin/bash
rm generated/*.ast
cd tests
find . -name "*.lua" -exec "bash" "-c" "cat {} | lua ../parse.lua | python3 ../transform.py > ../generated/{}.ast" ";"
