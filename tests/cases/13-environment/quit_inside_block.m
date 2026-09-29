% covers: 11 - quit; inside a block ends the script with code 0
for k = 1:3
    disp(k)
    if k == 2
        quit;
    end
end
disp(99)
