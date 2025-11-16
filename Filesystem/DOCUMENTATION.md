# BWFS - Documentation Template

## 1. Introducción

BWFS (Black and White FileSystem) es un sistema de archivos innovador implementado en espacio de usuario que utiliza imágenes en blanco y negro para almacenar información. Cada píxel representa un bit de datos, donde el blanco (255) representa 1 y el negro (0) representa 0. Este enfoque único permite visualizar físicamente el contenido del sistema de archivos mediante imágenes PNG.

### Problema

Los sistemas de archivos tradicionales almacenan datos en bloques binarios que no son directamente visualizables. BWFS aborda la necesidad de un sistema de archivos educativo que:

1. Permita visualizar físicamente el almacenamiento de datos
2. Demuestre conceptos de sistemas de archivos de manera tangible
3. Implemente operaciones POSIX estándar en espacio de usuario
4. Soporte persistencia y distribución de datos

### Solución Implementada

BWFS implementa un sistema de archivos completo usando:
- **FUSE** para operaciones en espacio de usuario
- **Imágenes PNG** para almacenamiento físico de datos
- **Sistema de i-nodos** para gestión de archivos
- **Bitmaps** para tracking de bloques e i-nodos libres
- **TCP/IP** para soporte de red distribuida

---

## 2. Ambiente de Desarrollo

### Hardware
- Arquitectura: x86_64
- RAM mínima: 2 GB
- Espacio en disco: 500 MB para desarrollo

### Software

#### Sistema Operativo
- GNU/Linux (Ubuntu 22.04 LTS recomendado)
- Kernel 5.x o superior con soporte FUSE

#### Herramientas de Desarrollo
- **Rust**: 1.70+ (2021 Edition)
- **Cargo**: Sistema de build de Rust
- **FUSE**: libfuse 2.9+ o 3.x
- **Git**: Control de versiones

#### Dependencias Rust
```toml
fuser = "0.14"           # Biblioteca FUSE para Rust
image = "0.24"           # Procesamiento de imágenes
ini = "1.3"              # Parsing de archivos INI
tokio = "1.35"           # Runtime asíncrono
serde = "1.0"            # Serialización
clap = "4.4"             # CLI parsing
```

#### Instalación del Ambiente

```bash
# Instalar Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Instalar FUSE (Ubuntu/Debian)
sudo apt-get install fuse libfuse-dev pkg-config

# Instalar FUSE (Fedora)
sudo dnf install fuse fuse-devel

# Clonar y compilar proyecto
git clone <repository>
cd Filesystem
cargo build --release
```

---

## 3. Estructuras de Datos y Funciones

### 3.1 Estructuras de Datos Principales

#### INode
Representa un archivo o directorio en el sistema.

```rust
pub struct INode {
    pub ino: u64,                    // Número único de i-nodo
    pub file_type: FileType,         // Tipo: archivo/directorio/symlink
    pub size: u64,                   // Tamaño en bytes
    pub nlink: u32,                  // Número de hard links
    pub uid: u32,                    // User ID del propietario
    pub gid: u32,                    // Group ID del propietario
    pub mode: u16,                   // Permisos (rwxrwxrwx)
    pub atime: SystemTime,           // Último acceso
    pub mtime: SystemTime,           // Última modificación
    pub ctime: SystemTime,           // Último cambio de metadata
    pub direct_blocks: [u32; 12],    // 12 bloques directos
    pub indirect_block: u32,         // Bloque indirecto simple
    pub double_indirect_block: u32,  // Bloque doblemente indirecto
}
```

**Funciones principales:**
- `new()`: Crea un nuevo i-nodo
- `get_block_number()`: Obtiene el número de bloque para un índice
- `set_block_number()`: Asigna un bloque a un índice

#### BlockStorage
Gestiona el almacenamiento en imágenes PNG.

```rust
pub struct BlockStorage {
    base_path: PathBuf,           // Ruta base de almacenamiento
    block_width: u32,             // Ancho del bloque en píxeles
    block_height: u32,            // Alto del bloque en píxeles
    bytes_per_block: usize,       // Capacidad en bytes
    total_blocks: u32,            // Número total de bloques
    fingerprint: String,          // Identificador del FS
}
```

**Funciones principales:**
- `read_block()`: Lee datos desde una imagen PNG
- `write_block()`: Escribe datos a una imagen PNG
- `init_block()`: Inicializa un nuevo bloque
- `verify_fingerprint()`: Verifica la identidad del filesystem

#### Bitmap
Tracking de recursos libres/usados.

```rust
pub struct Bitmap {
    bits: Vec<u8>,    // Vector de bits
    size: usize,      // Número total de bits
}
```

**Funciones principales:**
- `allocate()`: Encuentra y reserva el primer bit libre
- `deallocate()`: Libera un bit
- `is_set()`: Verifica si un bit está usado

#### DirEntry
Entrada de directorio.

```rust
pub struct DirEntry {
    pub ino: u64,              // Número de i-nodo
    pub name: String,          // Nombre del archivo
    pub file_type: FileType,   // Tipo de archivo
}
```

### 3.2 Funciones FUSE Implementadas

#### getattr
Obtiene atributos de un archivo o directorio.

```rust
fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr)
```
- Busca el i-nodo solicitado
- Convierte INode a FileAttr de FUSE
- Retorna atributos: tamaño, permisos, timestamps, etc.

#### open
Abre un archivo para lectura/escritura.

```rust
fn open(&mut self, _req: &Request, ino: u64, flags: i32, reply: ReplyOpen)
```
- Verifica que el archivo existe
- Asigna un file handle único
- Retorna el handle para operaciones futuras

#### read
Lee datos de un archivo.

```rust
fn read(&mut self, _req: &Request, ino: u64, fh: u64, offset: i64, 
        size: u32, flags: i32, lock_owner: Option<u64>, reply: ReplyData)
```
- Calcula qué bloques leer según offset y size
- Lee los bloques necesarios desde las imágenes
- Extrae el segmento solicitado
- Retorna los datos

#### write
Escribe datos a un archivo.

```rust
fn write(&mut self, _req: &Request, ino: u64, fh: u64, offset: i64,
         data: &[u8], write_flags: u32, flags: i32, 
         lock_owner: Option<u64>, reply: ReplyWrite)
```
- Calcula bloques necesarios
- Asigna nuevos bloques si es necesario
- Lee bloques existentes, modifica, y escribe de vuelta
- Actualiza tamaño del archivo y timestamps

#### create
Crea un nuevo archivo.

```rust
fn create(&mut self, req: &Request, parent: u64, name: &OsStr,
          mode: u32, umask: u32, flags: i32, reply: ReplyEntry)
```
- Verifica que el directorio padre existe
- Crea un nuevo i-nodo
- Añade entrada al directorio padre
- Retorna atributos del nuevo archivo

#### mkdir
Crea un nuevo directorio.

```rust
fn mkdir(&mut self, req: &Request, parent: u64, name: &OsStr,
         mode: u32, umask: u32, reply: ReplyEntry)
```
- Crea i-nodo de tipo directorio
- Inicializa con entradas . y ..
- Incrementa nlink del padre
- Añade entrada al directorio padre

#### readdir
Lista contenido de un directorio.

```rust
fn readdir(&mut self, _req: &Request, ino: u64, fh: u64,
           offset: i64, mut reply: ReplyDirectory)
```
- Obtiene lista de entradas del directorio
- Itera sobre las entradas desde el offset
- Llena el buffer de respuesta
- Maneja paginación si el buffer se llena

#### unlink
Elimina un archivo.

```rust
fn unlink(&mut self, _req: &Request, parent: u64, name: &OsStr,
          reply: fuser::ReplyEmpty)
```
- Busca la entrada en el directorio padre
- Decrementa nlink del i-nodo
- Si nlink llega a 0, libera los bloques
- Elimina el i-nodo

#### rmdir
Elimina un directorio.

```rust
fn rmdir(&mut self, _req: &Request, parent: u64, name: &OsStr,
         reply: fuser::ReplyEmpty)
```
- Verifica que el directorio está vacío
- Elimina el directorio del padre
- Libera el i-nodo
- Decrementa nlink del padre

#### rename
Renombra o mueve un archivo.

```rust
fn rename(&mut self, _req: &Request, parent: u64, name: &OsStr,
          newparent: u64, newname: &OsStr, flags: u32,
          reply: fuser::ReplyEmpty)
```
- Busca la entrada en el directorio origen
- Mueve la entrada al directorio destino
- Actualiza el nombre si cambió

#### statfs
Retorna estadísticas del filesystem.

```rust
fn statfs(&mut self, _req: &Request, ino: u64, reply: fuser::ReplyStatfs)
```
- Calcula bloques totales y libres
- Calcula i-nodos totales y libres
- Retorna capacidad, uso, y tamaño de bloque

#### fsync
Sincroniza datos al disco.

```rust
fn fsync(&mut self, _req: &Request, ino: u64, fh: u64,
         datasync: bool, reply: fuser::ReplyEmpty)
```
- Llama a save() para persistir metadata
- Asegura que todos los cambios están en disco

#### access
Verifica permisos de acceso.

```rust
fn access(&mut self, _req: &Request, ino: u64, mask: i32,
          reply: fuser::ReplyEmpty)
```
- Verifica que el archivo existe
- Valida permisos según la máscara solicitada

### 3.3 Algoritmos Clave

#### Conversión de Bytes a Píxeles

```rust
// Escribir: bytes → píxeles
for byte in data {
    for i in 0..8 {
        let bit = (byte >> (7 - i)) & 1;
        pixels.push(if bit == 1 { 255 } else { 0 });
    }
}

// Leer: píxeles → bytes
for chunk in pixels.chunks(8) {
    let mut byte = 0u8;
    for (i, &pixel) in chunk.iter().enumerate() {
        if pixel > 127 {
            byte |= 1 << (7 - i);
        }
    }
    data.push(byte);
}
```

#### Gestión de Bloques

```rust
// Calcular bloques necesarios para offset y tamaño
fn calculate_blocks(offset: usize, size: usize, block_size: usize) 
    -> (usize, usize) {
    let start_block = offset / block_size;
    let end_block = (offset + size + block_size - 1) / block_size;
    (start_block, end_block)
}
```

---

## 4. Instrucciones de Ejecución

### 4.1 Compilación

```bash
# Compilar en modo release
cargo build --release

# Los binarios estarán en:
# - target/release/mkfs.bwfs
# - target/release/mount.bwfs
```

### 4.2 Crear Filesystem

```bash
# Crear configuración (config.ini)
cat > config.ini << EOF
[filesystem]
name = MyBWFS
block_width = 1000
block_height = 1000
total_blocks = 100
total_inodes = 1024
storage_path = ./bwfs_data
fingerprint = BWFS_v1.0
tcp_port = 9000
EOF

# Crear el filesystem
./target/release/mkfs.bwfs -c config.ini
```

### 4.3 Montar Filesystem

```bash
# Crear punto de montaje
mkdir -p /tmp/bwfs_mount

# Montar (background)
./target/release/mount.bwfs -c config.ini /tmp/bwfs_mount

# O montar en foreground para debugging
./target/release/mount.bwfs -c config.ini -f /tmp/bwfs_mount
```

### 4.4 Usar el Filesystem

```bash
# Crear archivo
echo "Hello BWFS!" > /tmp/bwfs_mount/hello.txt

# Leer archivo
cat /tmp/bwfs_mount/hello.txt

# Crear directorio
mkdir /tmp/bwfs_mount/mydir

# Copiar archivos
cp /etc/hosts /tmp/bwfs_mount/mydir/

# Listar contenido
ls -la /tmp/bwfs_mount

# Ver estadísticas
df -h /tmp/bwfs_mount

# Ver una imagen de bloque (ejemplo)
display ./bwfs_data/block_00000001.png
```

### 4.5 Desmontar

```bash
# Desmontar
fusermount -u /tmp/bwfs_mount

# O con umount (requiere permisos)
sudo umount /tmp/bwfs_mount
```

### 4.6 Tests Automatizados

```bash
# Ejecutar script de tests
chmod +x test.sh
./test.sh
```

---

## 5. Actividades Realizadas por Estudiante

| Fecha | Estudiante | Actividad | Horas |
|-------|-----------|-----------|-------|
| 2025-11-01 | Juan Pérez | Diseño de arquitectura inicial | 4 |
| 2025-11-02 | Juan Pérez | Implementación de estructuras de datos | 6 |
| 2025-11-03 | María García | Implementación BlockStorage | 8 |
| 2025-11-04 | María García | Tests de almacenamiento en imágenes | 4 |
| 2025-11-05 | Juan Pérez | Implementación operaciones FUSE básicas | 10 |
| 2025-11-06 | María García | Implementación operaciones de directorio | 8 |
| 2025-11-07 | Juan Pérez | Sistema de persistencia | 6 |
| 2025-11-08 | María García | Módulo de red TCP/IP | 7 |
| 2025-11-09 | Juan Pérez | Debugging y fixes | 5 |
| 2025-11-10 | María García | Documentación y tests | 6 |
| 2025-11-11 | Ambos | Testing final y demo | 8 |

**Total horas Juan Pérez: 39**  
**Total horas María García: 33**  
**Total proyecto: 72 horas**

---

## 6. Autoevaluación

### 6.1 Estado Final

#### Funcionalidades Completadas ✓
- [x] mkfs.bwfs - Creación de filesystem
- [x] mount.bwfs - Montaje de filesystem
- [x] Operaciones FUSE básicas (getattr, open, read, write, create)
- [x] Operaciones de directorio (mkdir, rmdir, readdir, opendir)
- [x] Operaciones avanzadas (rename, unlink, access, statfs, flush, fsync)
- [x] Almacenamiento en imágenes B/N
- [x] Sistema de i-nodos
- [x] Bitmaps para tracking de recursos
- [x] Persistencia en disco
- [x] Configuración mediante INI
- [x] Fingerprint de filesystem

#### Limitaciones

1. **Bloques indirectos**: Solo se implementaron bloques directos (12 por archivo)
   - Tamaño máximo de archivo limitado a 12 × 125KB ≈ 1.5MB
   
2. **Performance**: Acceso más lento que filesystems nativos
   - Codificación/decodificación de píxeles añade overhead
   - PNG tiene compresión que afecta velocidad

3. **Red distribuida**: Implementación básica
   - Protocolo funcional pero sin replicación avanzada
   - Sin manejo de fallos de red

4. **lseek**: Delegado a FUSE, no implementado explícitamente

### 6.2 Reporte de Commits

```bash
# Generar reporte
git log --pretty=format:"%h - %an, %ar : %s" --graph
```

(Incluir output real del proyecto)

### 6.3 Autocalificación

| Ítem | Puntos | Calificación | Justificación |
|------|--------|--------------|---------------|
| **mkfs.bwfs** | 14% | 14/14 | Funcional, crea estructura correctamente |
| **mount.bwfs** | 15% | 15/15 | Monta FS, verifica fingerprint |
| **Funciones FUSE** | 26% | 24/26 | Implementadas 15/16 (falta lseek explícito) |
| - getattr | - | 10/10 | Funcional completamente |
| - create | - | 10/10 | Crea archivos correctamente |
| - open | - | 10/10 | Abre archivos sin problemas |
| - read | - | 10/10 | Lee datos correctamente |
| - write | - | 10/10 | Escribe y persiste datos |
| - rename | - | 10/10 | Renombra/mueve archivos |
| - mkdir | - | 10/10 | Crea directorios |
| - readdir | - | 10/10 | Lista contenido |
| - opendir | - | 10/10 | Abre directorios |
| - rmdir | - | 10/10 | Elimina directorios vacíos |
| - statfs | - | 10/10 | Retorna estadísticas |
| - fsync | - | 10/10 | Sincroniza a disco |
| - access | - | 10/10 | Verifica permisos |
| - unlink | - | 10/10 | Elimina archivos |
| - flush | - | 10/10 | Limpia buffers |
| - lseek | - | 8/10 | No implementado explícitamente |
| **Documentación** | 20% | 18/20 | Completa pero podría mejorar ejemplos |
| **Persistencia** | 25% | 23/25 | Funcional, falta optimización |
| **TOTAL** | **100%** | **94/100** | |

### 6.4 Problemas Encontrados

1. **Overhead de PNG**: La compresión PNG añade latencia
   - Solución parcial: usar nivel de compresión bajo
   
2. **Sincronización**: Race conditions iniciales con Mutex
   - Solucionado: Arc<Mutex<>> en todas las estructuras compartidas
   
3. **FUSE lifecycle**: Problemas con cleanup
   - Solucionado: AutoUnmount en opciones de montaje

---

## 7. Lecciones Aprendidas

### Para Futuros Estudiantes

1. **Empezar temprano**: FUSE tiene una curva de aprendizaje
   - Leer documentación de FUSE primero
   - Hacer prototipo simple antes del proyecto real

2. **Testing incremental**: Probar cada operación FUSE individualmente
   - Usar `strace` para debugging
   - Logs detallados son esenciales

3. **Gestión de estado**: El filesystem es altamente stateful
   - Usar estructuras inmutables donde sea posible
   - Cuidado con locks (deadlocks son comunes)

4. **Persistencia desde el inicio**: No dejar para el final
   - Diseñar formato de metadata temprano
   - Usar serialización (JSON/bincode) desde v1

5. **Performance no es obvia**: Medir antes de optimizar
   - El overhead de imágenes es significativo
   - Considerar cache de bloques frecuentes

6. **FUSE quirks**: Comportamiento no siempre intuitivo
   - Algunos syscalls llaman múltiples operaciones
   - `lookup` se llama muchas veces

7. **Rust ownership**: Puede ser challenging con FUSE
   - Arc<Mutex<>> es tu amigo
   - Evitar clones innecesarios

---

## 8. Bibliografía

1. **FUSE Documentation**  
   https://www.kernel.org/doc/html/latest/filesystems/fuse.html

2. **fuser - FUSE library for Rust**  
   https://docs.rs/fuser/latest/fuser/

3. **The Rust Programming Language**  
   Klabnik, S., & Nichols, C. (2023)  
   https://doc.rust-lang.org/book/

4. **Operating Systems: Three Easy Pieces**  
   Arpaci-Dusseau, R. H., & Arpaci-Dusseau, A. C.  
   Chapter 40: File System Implementation

5. **Linux System Programming**  
   Love, R. (2013). O'Reilly Media  
   Chapters on filesystems and FUSE

6. **image-rs Documentation**  
   https://docs.rs/image/latest/image/

7. **PNG Specification**  
   https://www.w3.org/TR/PNG/

8. **Tokio Async Runtime**  
   https://tokio.rs/

9. **Writing a File System from Scratch in Rust**  
   https://blog.carlosgaldino.com/writing-a-file-system-from-scratch-in-rust.html

10. **FUSE Tutorial**  
    Joseph J. Pfeiffer, Jr., Ph.D.  
    https://www.cs.nmsu.edu/~pfeiffer/fuse-tutorial/

---

## Apéndice A: Formato de Bloque

Cada bloque es una imagen PNG de NxN píxeles:

```
Bloque 0: Superblock
- 0-31: Fingerprint (string ASCII)
- 32-63: Versión
- 64-95: Total blocks
- 96-127: Total inodes
- 128+: Reservado

Bloques 1-N: Datos de usuario
- Contenido de archivos
- Estructuras de directorios
- Bloques indirectos (futuro)
```

## Apéndice B: Estructura de Metadata

```json
{
  "inodes": {
    "1": {
      "ino": 1,
      "file_type": "Directory",
      "size": 0,
      "nlink": 2,
      "uid": 1000,
      "gid": 1000,
      "mode": 493,
      "direct_blocks": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
    }
  },
  "directories": {
    "1": [
      {"ino": 1, "name": ".", "file_type": "Directory"},
      {"ino": 1, "name": "..", "file_type": "Directory"}
    ]
  },
  "next_ino": 2
}
```

## Apéndice C: Comandos Útiles

```bash
# Debug FUSE
strace -e trace=file,openat,read,write ls /tmp/bwfs_mount

# Ver syscalls
fusermount -u /tmp/bwfs_mount 2>&1 | less

# Logs detallados
RUST_LOG=debug ./target/release/mount.bwfs -c config.ini -f /tmp/bwfs_mount

# Verificar imagen de bloque
file ./bwfs_data/block_00000001.png
identify ./bwfs_data/block_00000001.png

# Benchmark
time dd if=/dev/zero of=/tmp/bwfs_mount/test bs=1M count=1
```
