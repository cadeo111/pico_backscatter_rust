#! bash
distrobox ls | grep -q thesis_data_processing
BOX_EXISTS=$?


if [[ $BOX_EXISTS ]]; then
 
distrobox create -i ubuntu:24.04 -n thesis_data_processing -Y
fi
