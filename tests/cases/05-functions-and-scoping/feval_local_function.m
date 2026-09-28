% covers: 13 - feval calls a function named by a char, here one local to the script
disp(feval('sq', 3))
function y = sq(x)
    y = x^2;
end
