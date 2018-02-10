local t = {1, 2}
t["0"] = 3

assert(t["0"] == 3)
assert(t[0] == 1)
assert(t[1] == 2)
