% covers: 1 - strcat joins char arguments and drops each one's trailing whitespace, where [] concatenation keeps it
disp(strcat('a', 'b', 'c'))
disp(strcat('a ', 'b'))
disp(['a ' 'b'])
