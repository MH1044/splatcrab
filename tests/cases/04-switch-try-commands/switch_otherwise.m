% covers: 1 - switch laid out one clause per line: a plain case, then otherwise when no case holds the value
% Item 1's switch as a script writes it, run for a value its first case
% holds and for one that no case holds.
for x = [1 7]
    switch x
        case 1
            disp('one')
        case {2, 3}
            disp('two or three')
        otherwise
            disp('other')
    end
end
