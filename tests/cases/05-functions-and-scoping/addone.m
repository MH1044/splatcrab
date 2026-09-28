function y = addone(x)
% Helper for path_test and exist_path_file: a function file in the working
% directory, which is on the path. It has no covers line and no .out, so the
% harness never runs it as a case.
y = x + 1;
end
