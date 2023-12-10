# PPU Notes

## Tile Data
* Contains tiles which describes what color a pixel will be
* Some tiles are used to describe objects while others are used to describe the window/bg

## Tile Map
* For rendering the background or window
* Each byte is an index into the tile data table

## OAM
* Contains the index into the tile data table for colors
* Contains coordinates to draw