#!/bin/bash
# Script de prueba para BWFS

set -e

echo "=== BWFS Test Script ==="
echo

# Colores para output
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Compilar el proyecto
echo "1. Compilando proyecto..."
cargo build --release
echo -e "${GREEN}✓ Compilación exitosa${NC}"
echo

# Crear filesystem
echo "2. Creando filesystem..."
./target/release/mkfs.bwfs -c config.ini
echo -e "${GREEN}✓ Filesystem creado${NC}"
echo

# Crear punto de montaje
MOUNT_POINT="/tmp/bwfs_test_$$"
mkdir -p "$MOUNT_POINT"
echo "3. Punto de montaje creado: $MOUNT_POINT"
echo

# Montar filesystem en background
echo "4. Montando filesystem..."
./target/release/mount.bwfs -c config.ini "$MOUNT_POINT" &
MOUNT_PID=$!
sleep 2

if mount | grep "$MOUNT_POINT" > /dev/null; then
    echo -e "${GREEN}✓ Filesystem montado${NC}"
else
    echo -e "${RED}✗ Error al montar filesystem${NC}"
    exit 1
fi
echo

# Test 1: Crear archivo
echo "5. Test 1: Crear y escribir archivo..."
echo "Hello BWFS!" > "$MOUNT_POINT/test.txt"
if [ -f "$MOUNT_POINT/test.txt" ]; then
    echo -e "${GREEN}✓ Archivo creado${NC}"
else
    echo -e "${RED}✗ Error al crear archivo${NC}"
fi

# Test 2: Leer archivo
echo "6. Test 2: Leer archivo..."
CONTENT=$(cat "$MOUNT_POINT/test.txt")
if [ "$CONTENT" = "Hello BWFS!" ]; then
    echo -e "${GREEN}✓ Contenido correcto: $CONTENT${NC}"
else
    echo -e "${RED}✗ Contenido incorrecto: $CONTENT${NC}"
fi
echo

# Test 3: Crear directorio
echo "7. Test 3: Crear directorio..."
mkdir "$MOUNT_POINT/testdir"
if [ -d "$MOUNT_POINT/testdir" ]; then
    echo -e "${GREEN}✓ Directorio creado${NC}"
else
    echo -e "${RED}✗ Error al crear directorio${NC}"
fi

# Test 4: Crear archivo en subdirectorio
echo "8. Test 4: Crear archivo en subdirectorio..."
echo "Nested file" > "$MOUNT_POINT/testdir/nested.txt"
if [ -f "$MOUNT_POINT/testdir/nested.txt" ]; then
    echo -e "${GREEN}✓ Archivo anidado creado${NC}"
else
    echo -e "${RED}✗ Error al crear archivo anidado${NC}"
fi
echo

# Test 5: Listar contenido
echo "9. Test 5: Listar contenido..."
ls -la "$MOUNT_POINT"
echo

# Test 6: Estadísticas del filesystem
echo "10. Test 6: Estadísticas del filesystem..."
df -h "$MOUNT_POINT"
echo

# Test 7: Renombrar archivo
echo "11. Test 7: Renombrar archivo..."
mv "$MOUNT_POINT/test.txt" "$MOUNT_POINT/renamed.txt"
if [ -f "$MOUNT_POINT/renamed.txt" ] && [ ! -f "$MOUNT_POINT/test.txt" ]; then
    echo -e "${GREEN}✓ Archivo renombrado${NC}"
else
    echo -e "${RED}✗ Error al renombrar archivo${NC}"
fi
echo

# Test 8: Eliminar archivo
echo "12. Test 8: Eliminar archivo..."
rm "$MOUNT_POINT/renamed.txt"
if [ ! -f "$MOUNT_POINT/renamed.txt" ]; then
    echo -e "${GREEN}✓ Archivo eliminado${NC}"
else
    echo -e "${RED}✗ Error al eliminar archivo${NC}"
fi
echo

# Test 9: Escribir archivo más grande
echo "13. Test 9: Escribir archivo grande..."
dd if=/dev/urandom of="$MOUNT_POINT/large.dat" bs=1K count=100 2>/dev/null
if [ -f "$MOUNT_POINT/large.dat" ]; then
    SIZE=$(stat -f%z "$MOUNT_POINT/large.dat" 2>/dev/null || stat -c%s "$MOUNT_POINT/large.dat")
    echo -e "${GREEN}✓ Archivo grande creado (${SIZE} bytes)${NC}"
else
    echo -e "${RED}✗ Error al crear archivo grande${NC}"
fi
echo

# Limpiar
echo "14. Limpiando..."
fusermount -u "$MOUNT_POINT" 2>/dev/null || umount "$MOUNT_POINT" 2>/dev/null || true
rmdir "$MOUNT_POINT"
echo -e "${GREEN}✓ Filesystem desmontado${NC}"
echo

echo "=== Tests completados ==="
echo "Revisa las imágenes en ./bwfs_data/ para ver el almacenamiento en píxeles"
