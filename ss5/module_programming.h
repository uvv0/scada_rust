#ifndef MODULE_PROGRAMMING_H
#define MODULE_PROGRAMMING_H

#include <QByteArray>
#include <QList>
#include <QString>
#include <QVector>
#include <QtGlobal>

/* Формирование и разбор ELAM Modbus без привязки к COM/UDP-транспорту. */
namespace ModuleProgramming
{
enum {
    SlotCount=20, SlotSize=4096, HeaderSize=10,
    DataBase=10000, SlotRegister=12048, LengthRegister=12049,
    CommandRegister=12050, StatusRegister=12051, ResultRegister=12052,
    CrcRegister=12053, VerifyTokenRegister=12054, ConfirmTokenRegister=12055,
    MaxWriteRegisters=123,
    CommandVerify=0xA501, CommandWrite=0xA502,
    CommandStart=0xA503, CommandStop=0xA504,
    StatusIdle=0, StatusVerifying=1, StatusVerified=2, StatusWriting=3,
    StatusWritten=4, StatusError=5, StatusStarting=6, StatusRunning=7,
    StatusStopping=8, StatusStopped=9
};

struct ImageInfo {
    quint16 storedCrc, calculatedCrc, entryOffset, bodySize, type, version;
    quint32 protectedEnd;
};
struct OperationStatus {
    quint16 status, result, crc, slot, length;
};

quint16 crc16(const QByteArray &data);
quint16 crc16(const char *data, int size, quint16 initial=0xFFFF);
bool inspectImage(const QByteArray &image, int slot, ImageInfo *info,
                  QString *errorText);
bool loadImage(const QString &fileName, int slot, QByteArray *image,
               ImageInfo *info, QString *errorText);
QVector<quint16> imageWords(const QByteArray &image);
QList<QByteArray> makeDataWriteFrames(const QByteArray &image,
    quint16 station=301, int registersPerFrame=MaxWriteRegisters);
QByteArray makeReadHoldingFrame(quint16 address, quint16 count,
                                quint16 station=301);
QByteArray makeWriteSingleFrame(quint16 address, quint16 value,
                                quint16 station=301);
QByteArray makeWriteMultipleFrame(quint16 address,
    const QVector<quint16> &values, quint16 station=301);
QByteArray makeSelectFrame(quint16 slot, quint16 length, quint16 station=301);
QByteArray makeVerifyFrame(quint16 station=301);
QByteArray makeStatusReadFrame(quint16 station=301);
QByteArray makeConfirmFrame(quint16 token, quint16 station=301);
QByteArray makeWriteFrame(quint16 station=301);
QByteArray makeStartFrame(quint16 station=301);
QByteArray makeStopFrame(quint16 station=301);
bool parseReadHoldingResponse(const QByteArray &frame,
    QVector<quint16> *values, QString *errorText);
bool parseOperationStatus(const QByteArray &frame, OperationStatus *status,
                          QString *errorText);
bool isWriteAcknowledge(const QByteArray &request, const QByteArray &response,
                        QString *errorText);
quint16 expectedToken(quint16 crc, quint16 slot, quint16 length);
QString resultText(quint16 result);
QString statusText(quint16 status);
}
#endif
