% covers: 4 - an error inside a for body reports the body's line (4), not the loop header's (2)
for k = 1:3
    disp(k)
    [1 2] * [3 4]
end
