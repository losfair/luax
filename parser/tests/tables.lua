local t = {1, 2}
t["1"] = 3

assert(t["1"] == 3)
assert(t[1] == 1)
assert(t[2] == 2)
