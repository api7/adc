set -e

go install github.com/incu6us/goimports-reviser/v2@latest

PROJECT_NAME=github.com/api7/adc
while IFS= read -r -d '' file; do
  goimports-reviser  -file-path "$file" -project-name $PROJECT_NAME
done <   <(find . -name '*.go' -print0)
