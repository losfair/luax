function fib(n)
    while n == 0 do
        return 0
    end
    while n == 1 do
        return 1
    end
    return fib(n - 1) + fib(n - 2)
end

print(fib(35))
