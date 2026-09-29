% covers: 11 - (Scope, histc) histc counts the values with edges(k) <= x < edges(k+1), one count per edge
% No value sits on an edge, so the last count, of the values equal to
% edges(end), is 0.
disp(histc([1.5 2.5 2.5 3.5], 1:4))
