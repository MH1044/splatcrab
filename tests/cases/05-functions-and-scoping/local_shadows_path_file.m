% covers: 14 - a function local to the script comes before a file on the path: this addone, not addone.m
disp(addone(1))
function y = addone(x)
y = x + 100;
end
