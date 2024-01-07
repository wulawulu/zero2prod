# 开启调试模式
set -x

# 任何命令执行失败都将导致脚本终止执行
set -eo pipefail

docker restart pg
docker restart redis
ulimit -n 1000000


>&2 echo "Ready to go!"
