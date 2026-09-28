% covers: 5 - concatenation is char if any operand is char, logical only if all are
disp(['a' 66])
disp([65 'a'])
disp(class([true 2]))
disp(class([true false]))
disp(class(['a' true]))
