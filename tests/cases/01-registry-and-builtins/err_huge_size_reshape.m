% covers: 14 - reshape with a huge size argument is a clean error, not a panic
reshape(1:6, 1e10, 1e10)
