#ifndef WEB_PROGRAMMING_H
#define WEB_PROGRAMMING_H

#include <QByteArray>
#include <QList>
#include <QString>
#include <QVector>
#include <QtGlobal>

namespace WebProgramming
{
enum {
    SlotCount=254, SlotDataSize=65472, ChunkSize=4096,
    SlotRegister=12056, TotalHiRegister=12057,
    OffsetHiRegister=12059, CommandRegister=12085,
    StatusRegister=12086,
    CommandBegin=0xA511, CommandChunk=0xA512, CommandCommit=0xA513,
    StatusIdle=0, StatusErasing=1, StatusReady=2, StatusWriting=3,
    StatusChunkOk=4, StatusComplete=5, StatusError=6
};

struct Status {
    quint16 status, result;
    quint32 written, crc32;
};

quint32 crc32(const QByteArray &data);
quint16 contentTypeForFile(const QString &fileName);
QString defaultPathForFile(const QString &fileName);
QByteArray makeBeginConfigFrame(quint16 slot, quint32 total,
    quint16 contentType, quint32 crc, const QString &path,
    quint16 station=301);
QByteArray makeCommandFrame(quint16 command, quint16 station=301);
QList<QByteArray> makeChunkDataFrames(const QByteArray &chunk,
    quint16 station=301);
QByteArray makeChunkMetaFrame(quint32 offset, quint16 length,
    quint16 station=301);
QByteArray makeStatusReadFrame(quint16 station=301);
bool parseStatus(const QByteArray &frame, Status *status, QString *errorText);
QString statusText(quint16 status);
QString resultText(quint16 result);
}
#endif
