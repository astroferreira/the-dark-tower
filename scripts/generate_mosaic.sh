#!/bin/bash
# Generate 9 random worlds and combine into a 3x3 mosaic

set -e

# Configuration
OUTPUT_DIR="${OUTPUT_DIR:-./mosaic_output}"
MOSAIC_NAME="${MOSAIC_NAME:-world_mosaic.png}"
WIDTH="${WIDTH:-512}"
HEIGHT="${HEIGHT:-256}"
WORLD_STYLE="${WORLD_STYLE:-continental}"

# Create output directory
mkdir -p "$OUTPUT_DIR"

echo "Generating 9 random worlds..."
echo "Output directory: $OUTPUT_DIR"
echo "Map size: ${WIDTH}x${HEIGHT}"
echo "World style: $WORLD_STYLE"
echo ""

# Array to store generated filenames
declare -a IMAGES

# Generate 9 worlds with random seeds
for i in {1..9}; do
    # Generate random seed
    SEED=$RANDOM$RANDOM

    echo "[$i/9] Generating world with seed $SEED..."

    # Run the generator
    cargo run --release -- \
        --width "$WIDTH" \
        --height "$HEIGHT" \
        --seed "$SEED" \
        --world-style "$WORLD_STYLE" \
        --export-base-map \
        --headless

    # Move the generated file to output directory
    SRC_FILE="world_base_${SEED}.png"
    DST_FILE="${OUTPUT_DIR}/world_${i}_seed_${SEED}.png"

    if [ -f "$SRC_FILE" ]; then
        mv "$SRC_FILE" "$DST_FILE"
        IMAGES+=("$DST_FILE")
        echo "  -> Saved to $DST_FILE"
    else
        echo "  ERROR: Expected output file $SRC_FILE not found"
        exit 1
    fi

    echo ""
done

echo "All 9 worlds generated. Creating mosaic..."

# Check if ImageMagick is available
if command -v montage &> /dev/null; then
    # Create 3x3 mosaic using ImageMagick montage
    montage "${IMAGES[@]}" \
        -tile 3x3 \
        -geometry +2+2 \
        -background black \
        "${OUTPUT_DIR}/${MOSAIC_NAME}"

    echo "Mosaic created: ${OUTPUT_DIR}/${MOSAIC_NAME}"
elif command -v convert &> /dev/null; then
    # Alternative using convert with append
    echo "Using ImageMagick convert (slower than montage)..."

    # Create rows
    convert "${IMAGES[0]}" "${IMAGES[1]}" "${IMAGES[2]}" +append "${OUTPUT_DIR}/row1.png"
    convert "${IMAGES[3]}" "${IMAGES[4]}" "${IMAGES[5]}" +append "${OUTPUT_DIR}/row2.png"
    convert "${IMAGES[6]}" "${IMAGES[7]}" "${IMAGES[8]}" +append "${OUTPUT_DIR}/row3.png"

    # Stack rows
    convert "${OUTPUT_DIR}/row1.png" "${OUTPUT_DIR}/row2.png" "${OUTPUT_DIR}/row3.png" \
        -append "${OUTPUT_DIR}/${MOSAIC_NAME}"

    # Cleanup temp files
    rm -f "${OUTPUT_DIR}/row1.png" "${OUTPUT_DIR}/row2.png" "${OUTPUT_DIR}/row3.png"

    echo "Mosaic created: ${OUTPUT_DIR}/${MOSAIC_NAME}"
else
    echo ""
    echo "WARNING: ImageMagick not found. Individual world images are saved in $OUTPUT_DIR"
    echo "To create the mosaic manually, install ImageMagick and run:"
    echo "  montage ${OUTPUT_DIR}/world_*.png -tile 3x3 -geometry +2+2 ${OUTPUT_DIR}/${MOSAIC_NAME}"
fi

echo ""
echo "Done! Individual maps saved in: $OUTPUT_DIR/"
echo "Seeds used can be found in the filenames."
