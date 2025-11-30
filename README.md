# BWFS - Black and White FileSystem

## Descripción

BWFS (Black and White FileSystem) es un sistema de archivos en espacio de usuario que utiliza imágenes en blanco y negro para almacenar datos. Cada píxel representa un bit de información (blanco=1, negro=0), permitiendo almacenar archivos en formato de imagen PNG.

## Características

- ✓ Sistema de archivos basado en FUSE para GNU/Linux
- ✓ Almacenamiento en imágenes blanco y negro (1000x1000 px por bloque)
- ✓ Sistema de i-nodos para indexación de bloques
- ✓ Soporte para operaciones POSIX estándar
- ✓ Persistencia en disco mediante imágenes PNG
- ✓ Soporte para red distribuida mediante TCP/IP
- ✓ Configuración mediante archivos INI

## Estructura del Proyecto

```
Filesystem/
├── Cargo.toml              # Workspace principal
├── bwfs/                   # Biblioteca principal
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs          # Módulo principal
│       ├── config.rs       # Configuración del FS
│       ├── fs.rs           # Implementación FUSE
│       ├── inode.rs        # Estructuras de i-nodos
│       ├── storage.rs      # Almacenamiento en imágenes
│       └── network.rs      # Comunicación TCP/IP
├── mkfs-bwfs/              # Herramienta de creación
│   ├── Cargo.toml
│   └── src/
│       └── main.rs
├── mount-bwfs/             # Herramienta de montaje
│   ├── Cargo.toml
│   └── src/
│       └── main.rs
└── config.ini              # Configuración de ejemplo
```

## Dependencias

- Rust 2021 Edition
- fuser - Biblioteca FUSE para Rust
- image - Procesamiento de imágenes
- ini - Parsing de archivos INI
- tokio - Runtime asíncrono para networking
- serde/serde_json - Serialización
- clap - Parsing de argumentos CLI

## Compilación

### Compilar todo el workspace

```bash
cargo build --release
```

### Compilar binarios individuales

```bash
# Crear filesystem
cargo build --release --bin mkfs_bwfs

# Montar filesystem
cargo build --release --bin mount.bwfs
```

Los binarios compilados estarán en: `target/release/`

## Instalación

### En Linux (x86_64)

```bash
# Instalar FUSE si no está instalado
sudo apt-get install fuse libfuse-dev

# Compilar el proyecto
cargo build --release

# Copiar binarios al sistema (opcional)
sudo cp target/release/mkfs_bwfs /usr/local/bin/
sudo cp target/release/mount.bwfs /usr/local/bin/
```

## Uso

### 1. Crear un nuevo filesystem

```bash
# Editar config.ini con los parámetros deseados
./target/release/mkfs_bwfs -c config.ini
```

Esto creará la estructura del filesystem en la ruta especificada en `storage_path`.

### 2. Montar el filesystem

```bash
# Crear punto de montaje
mkdir -p /tmp/bwfs_mount

# Montar el filesystem
./target/release/mount.bwfs -c config.ini /tmp/bwfs_mount

# Montar en primer plano (para debugging)
./target/release/mount.bwfs -c config.ini -f /tmp/bwfs_mount
```

### 3. Usar el filesystem

```bash
# Crear archivos
echo "Hello BWFS!" > /tmp/bwfs_mount/hello.txt

# Crear directorios
mkdir /tmp/bwfs_mount/test_dir

# Copiar archivos
cp /etc/hosts /tmp/bwfs_mount/test_dir/

# Listar contenido
ls -la /tmp/bwfs_mount

# Leer archivos
cat /tmp/bwfs_mount/hello.txt
```

### 4. Desmontar el filesystem

```bash
# Desmontar
fusermount -u /tmp/bwfs_mount

# O con umount (requiere permisos)
sudo umount /tmp/bwfs_mount
```

## Configuración

El archivo `config.ini` tiene la siguiente estructura:

```ini
[filesystem]
name = MyBWFS                    # Nombre del filesystem
block_width = 1000               # Ancho del bloque en píxeles (max 1000)
block_height = 1000              # Alto del bloque en píxeles (max 1000)
total_blocks = 100               # Número total de bloques
total_inodes = 1024              # Número total de i-nodos
storage_path = ./bwfs_data       # Ruta de almacenamiento
fingerprint = BWFS_v1.0          # Identificador del filesystem
tcp_port = 9000                  # Puerto TCP para red distribuida

[network]
# Nodos distribuidos opcionales
# node1 = 192.168.1.100:9000
# node2 = 192.168.1.101:9000
```

### Cálculo de Capacidad

Capacidad por bloque = (width × height) / 8 bytes

Ejemplo con bloques de 1000×1000 px:
- Bytes por bloque: 125,000 bytes (≈122 KB)
- 100 bloques: ≈12.2 MB
- 1000 bloques: ≈122 MB

## Operaciones FUSE Implementadas

### Básicas
- ✓ `getattr` - Obtener atributos de archivo/directorio
- ✓ `open` - Abrir archivo
- ✓ `read` - Leer datos de archivo
- ✓ `write` - Escribir datos a archivo
- ✓ `create` - Crear nuevo archivo
- ✓ `access` - Verificar permisos de acceso
- ✓ `flush` - Limpiar buffer de escritura
- ✓ `fsync` - Sincronizar datos al disco

### Directorios
- ✓ `mkdir` - Crear directorio
- ✓ `rmdir` - Eliminar directorio
- ✓ `readdir` - Leer contenido de directorio
- ✓ `opendir` - Abrir directorio

### Avanzadas
- ✓ `rename` - Renombrar/mover archivo
- ✓ `unlink` - Eliminar archivo
- ✓ `statfs` - Obtener estadísticas del filesystem
- ⚠️ `lseek` - Buscar en archivo (implementado por FUSE)

## Almacenamiento en Imágenes

Cada bloque del filesystem se almacena como una imagen PNG en blanco y negro:

- **Píxel blanco (255)** = bit 1
- **Píxel negro (0)** = bit 0

Los archivos se nombran secuencialmente: `block_00000000.png`, `block_00000001.png`, etc.

### Ejemplo de conversión

Byte `01001000` (letra 'H' en ASCII) se representa como:
```
Píxeles: Negro Blanco Negro Negro Blanco Negro Negro Negro
Colores:   0     255    0    0    255   0    0    0
```

## Arquitectura

### Capas del Sistema

```
┌─────────────────────────────────────┐
│      Aplicaciones de Usuario        │
├─────────────────────────────────────┤
│           API POSIX                 │
├─────────────────────────────────────┤
│      FUSE (Kernel Module)           │
├─────────────────────────────────────┤
│      BWFS (User Space)              │
│  ┌─────────────────────────────┐   │
│  │  Filesystem Operations      │   │
│  ├─────────────────────────────┤   │
│  │  INode Management           │   │
│  ├─────────────────────────────┤   │
│  │  Block Storage Manager      │   │
│  ├─────────────────────────────┤   │
│  │  Image I/O Layer            │   │
│  └─────────────────────────────┘   │
├─────────────────────────────────────┤
│      PNG Image Files (Disk)         │
└─────────────────────────────────────┘
```

### Estructuras de Datos

#### INode
```rust
struct INode {
    ino: u64,                    // Número de i-nodo
    file_type: FileType,         // Tipo de archivo
    size: u64,                   // Tamaño en bytes
    nlink: u32,                  // Número de enlaces
    uid: u32,                    // User ID
    gid: u32,                    // Group ID
    mode: u16,                   // Permisos
    atime: SystemTime,           // Tiempo de acceso
    mtime: SystemTime,           // Tiempo de modificación
    ctime: SystemTime,           // Tiempo de cambio
    direct_blocks: [u32; 12],    // Bloques directos
    indirect_block: u32,         // Bloque indirecto
    double_indirect_block: u32,  // Bloque doblemente indirecto
}
```

#### Directory Entry
```rust
struct DirEntry {
    ino: u64,           // Número de i-nodo
    name: String,       // Nombre del archivo
    file_type: FileType,// Tipo de archivo
}
```

## Red Distribuida

BWFS soporta almacenamiento distribuido mediante TCP/IP:

1. Configurar nodos en `config.ini`:
```ini
[network]
node1 = 192.168.1.100:9000
node2 = 192.168.1.101:9000
```

2. Los bloques se pueden replicar o distribuir entre nodos
3. Comunicación mediante protocolo JSON sobre TCP

## Testing

### Tests Básicos

```bash
# Crear y montar filesystem
./target/release/mkfs_bwfs -c config.ini
mkdir -p /tmp/bwfs_test
./target/release/mount.bwfs -c config.ini /tmp/bwfs_test

# Test de escritura/lectura
echo "Test BWFS" > /tmp/bwfs_test/test.txt
cat /tmp/bwfs_test/test.txt

# Test de directorios
mkdir /tmp/bwfs_test/dir1
mkdir /tmp/bwfs_test/dir1/dir2
ls -R /tmp/bwfs_test

# Test de archivos grandes
dd if=/dev/urandom of=/tmp/bwfs_test/large.dat bs=1M count=1

# Verificar integridad
md5sum /tmp/bwfs_test/large.dat

# Limpiar
fusermount -u /tmp/bwfs_test
```

## Limitaciones Conocidas

1. **Bloques indirectos**: Actualmente solo se implementan bloques directos (12 bloques por archivo)
2. **Máximo tamaño de archivo**: Limitado por el número de bloques directos
3. **Performance**: El acceso a disco mediante imágenes PNG es más lento que sistemas de archivos nativos
4. **Compresión**: Las imágenes PNG se comprimen, lo que puede afectar el rendimiento

## Troubleshooting

### Error: "Device or resource busy"
```bash
# Forzar desmontaje
fusermount -uz /tmp/bwfs_mount
# o
sudo umount -l /tmp/bwfs_mount
```

### Error: "FUSE not found"
```bash
# Instalar FUSE
sudo apt-get install fuse libfuse-dev
```

### Error: "Permission denied"
```bash
# Agregar usuario al grupo fuse
sudo usermod -a -G fuse $USER
# Cerrar sesión y volver a iniciar
```

### Logs de debugging
```bash
# Ejecutar con logs habilitados
RUST_LOG=debug ./target/release/mount.bwfs -c config.ini -f /tmp/bwfs_mount
```

## Trabajo Futuro

- [ ] Implementar bloques indirectos para archivos grandes
- [ ] Optimizar I/O de imágenes (cache, buffering)
- [ ] Implementar compresión opcional de datos
- [ ] Mejorar la distribución de bloques en red
- [ ] Agregar journaling para recuperación de fallos
- [ ] Implementar enlaces simbólicos
- [ ] Soporte para atributos extendidos (xattr)
- [ ] Herramientas de diagnóstico (fsck.bwfs)

## Contribuciones

Este proyecto es parte de una asignación académica para el curso de Sistemas Operativos Avanzados del TEC.

## Licencia

Proyecto académico - TEC 2025

## Autores

- Equipo BWFS
- Profesor: Kevin Moraga (kmoragas@ic-itcr.ac.cr)
