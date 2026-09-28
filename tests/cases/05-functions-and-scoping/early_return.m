% covers: 7 - return leaves the function at once, with its output as assigned so far
disp(early(5)); disp(early(-5))
function r = early(x)
r = 0; if x > 0, r = 1; return; end
r = -1;
end
