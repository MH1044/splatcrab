% covers: 5 - a function calls itself, each call with its own n
fprintf('%d\n', fact(10))
function r = fact(n)
if n <= 1, r = 1; else, r = n * fact(n - 1); end
end
