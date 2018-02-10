function generate(a)
    return function(b)
        return a + b
    end
end

local f = generate(5)
print(f(3))
