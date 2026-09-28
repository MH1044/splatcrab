% covers: 3 - break inside a switch leaves the enclosing for loop
for k = 1:5, switch k, case 3, break, end, fprintf('%d', k); end; fprintf('\n')
