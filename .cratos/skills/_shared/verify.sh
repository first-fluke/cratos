#!/bin/bash
# Cratos 검증 스크립트

set -e

echo "=== Cratos 검증 시작 ==="

# 색상 정의
RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'

# 결과 카운터
PASSED=0
FAILED=0

check() {
    local name="$1"
    local cmd="$2"

    echo -n "검사: $name... "
    if eval "$cmd" > /dev/null 2>&1; then
        echo -e "${GREEN}PASS${NC}"
        ((PASSED++))
    else
        echo -e "${RED}FAIL${NC}"
        ((FAILED++))
    fi
}

# Rust 검증
if [ -f "Cargo.toml" ]; then
    echo ""
    echo "--- Rust 검증 ---"
    check "cargo check" "cargo check"
    check "cargo test" "cargo test"
    check "cargo clippy" "cargo clippy -- -D warnings"
    check "cargo fmt" "cargo fmt -- --check"
fi

# 설정 파일 검증
echo ""
echo "--- 설정 파일 검증 ---"
check ".agent/mcp.json 존재" "[ -f .agent/mcp.json ]"
check "user-preferences.yaml 존재" "[ -f .agent/config/user-preferences.yaml ]"

# 스킬 검증
echo ""
echo "--- 스킬 검증 ---"
for skill_dir in .cratos/skills/*/; do
    skill_name=$(basename "$skill_dir")
    if [ "$skill_name" != "_shared" ]; then
        check "$skill_name SKILL.md" "[ -f ${skill_dir}SKILL.md ]"
    fi
done

# 결과 요약
echo ""
echo "=== 검증 결과 ==="
echo -e "통과: ${GREEN}${PASSED}${NC}"
echo -e "실패: ${RED}${FAILED}${NC}"

if [ $FAILED -gt 0 ]; then
    exit 1
fi
