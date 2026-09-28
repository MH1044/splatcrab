% covers: 5 - uncaught, error('id', fmt, ...) reports its formatted message, not its identifier (QA D9)
% Script mode reports Error: Line N: <message>. Before this cycle the
% message reported was the identifier.
error('MyPkg:myid', 'Value %d bad', 7)
