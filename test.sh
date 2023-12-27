#!/bin/bash
changedFiles=$(git diff origin/main.. --name-only)
echo ${changedFiles}
mlc_installed=0
for file in $changedFiles; do
    if [[ ".md" == ".${file##*.}" ]]; then
        if [[ $mlc_installed == 0 ]]; then
            # we install mlc instead of adding to devctr because
            # it is small and hardly takes 2-3mins to install it
            cargo install mlc
            mlc_installed=1
        fi
        # limit the links to validate by creatinga new file
        # having diff of the modified code and running mlc on
        # the new temp file.
        modified_file_dirname=$(dirname "$file")
        modified_file_basename=$(basename "$file")
        temp_validation_file=$modified_file_dirname/validate_$modified_file_basename
        echo ${temp_validation_file};
        git diff origin/main.. ${file} | grep + > $temp_validation_file
        cat ${temp_validation_file}
        mlc $temp_validation_file
        # rm $temp_validation_file
    fi
done
