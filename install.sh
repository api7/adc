#!/usr/bin/env bash

ARCH="$(uname -m)"
OS="$(uname)"

# convert to standard arch names used in files
if [ "x${ARCH}" = "xx86_64" ]; then
    ARCH="amd64"
fi

# convert to standard os names used in files
# TODO: support Windows
if [ "x${OS}" = "xDarwin" ]; then
    OS="darwin"
else
    OS="linux"
fi

# either specify the version in an environment variable or get the latest from GitHub
if [ "x${ADC_VERSION}" = "x" ]; then
    ADC_VERSION=$(curl -L -s https://github.com/api7/adc/releases/latest |
        grep "adc/releases/tag/" | head -1 | awk -F '"' '{print $4}' |
        awk -F '/' '{print $NF}')
fi

if [ "x${ADC_VERSION}" = "x" ]; then
    printf "Unable to find the latest version of ADC. Please set the ADC_VERSION environment variable and try again. For example, export ADC_VERSION=0.5.0\n"
    exit 1
fi

# if version has v in prefix, remove it
ADC_VERSION=${ADC_VERSION#v}

FILENAME="adc_${ADC_VERSION}_${OS}_${ARCH}.tar.gz"

# example download URL format: https://github.com/api7/adc/releases/download/v0.5.0/adc_0.5.0_darwin_arm64.tar.gz
URL="https://github.com/api7/adc/releases/download/v${ADC_VERSION}/${FILENAME}"

printf "Downloading ADC v${ADC_VERSION} for ${OS} ${ARCH}...\n\n"
# printf "Download URL: %s\n" "$URL"

curl -L ${URL} -o ${PWD}/adc.tar.gz
if [ $? -ne 0 ]; then
    echo "Error downloading ADC. Please check your internet connection and try again."
    exit 1
fi

# temporary folder name to extract the downloaded file
TEMP_FOLDER_NAME=$(tr -dc A-Za-z0-9 </dev/urandom 2>/dev/null | head -c 16)
if [ -z "$TEMP_FOLDER_NAME" ]; then
    TEMP_FOLDER_NAME="TEMP_ADC_FOLDER"
fi

mkdir $TEMP_FOLDER_NAME

printf "\nExtracting ADC to temporary folder %s...\n" "$TEMP_FOLDER_NAME"

tar -xzf "${PWD}/adc.tar.gz" -C "${PWD}/${TEMP_FOLDER_NAME}"
if [ $? -ne 0 ]; then
    echo "Error extracting ADC. The downloaded file might be corrupted. Please try again and make sure that you are installing the correct version."
    exit 1
fi

INSTALL_DIR=${ADC_DIR}
if [ -z "$INSTALL_DIR" ]; then
    INSTALL_DIR="/usr/local/bin"
fi

printf "Installing ADC in $INSTALL_DIR...\n"

WHOAMI=$(whoami)

# install adc binary, use sudo if user doesn't have permission to install in INSTALL_DIR
if mv "${PWD}/$TEMP_FOLDER_NAME/adc" "$INSTALL_DIR/adc" >/dev/null 2>&1; then
    echo "ADC installed successfully!"
else
    if sudo mv ${PWD}/$TEMP_FOLDER_NAME/adc $INSTALL_DIR/adc; then
        echo "ADC installed successfully with sudo permissions!"
    else
        echo "Unable to install ADC. Please check the permissions of the user $WHOAMI for the directory $INSTALL_DIR."
        exit 1
    fi
fi

# clean up temporary files
printf "Removing temporary files...\n"
rm -rf adc.tar.gz ${PWD}/$TEMP_FOLDER_NAME/

printf "Done!\n"
