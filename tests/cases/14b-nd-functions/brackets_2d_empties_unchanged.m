% covers: 11 - a bracket with no N-D operand keeps today's 2-D rule exactly: its empties, shapes and classes, cell2mat alike
disp(size([1:0]))
disp(size([zeros(1, 0)]))
disp(size([zeros(1, 0), zeros(0, 1)]))
disp(size([zeros(0, 3); zeros(0, 2)]))
disp(size([zeros(3, 0), zeros(3, 0), 1:0]))
disp(size(cell2mat({zeros(3, 0), zeros(2, 0)})))
disp(class([zeros(1, 0); true]))
