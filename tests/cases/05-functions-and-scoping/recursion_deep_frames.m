% covers: 17 - 400 nested calls fit the stack: hundreds of frames run to a result, short of the limit of 500
disp(deep(400))
function r = deep(n)
if n <= 1, r = 1; else, r = 1 + deep(n - 1); end
end
