% covers: 11 - exit(n) inside try is not caught: the script ends with code n
try
    exit(4);
catch
    disp(9)
end
disp(8)
