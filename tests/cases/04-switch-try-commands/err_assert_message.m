% covers: 8 - assert(true) passes, and a failed assert with a format reports the formatted message
assert(true); assert(1 == 2, 'nope %d', 3)
