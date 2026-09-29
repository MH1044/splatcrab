% covers: 11 - exit(n) inside a function ends the whole script with code n
disp(1)
f()
disp(2)
function f()
    exit(5);
end
